use core::str::from_utf8;
use defmt::*;

use embassy_time::Timer;
use embassy_time::Duration;
use embassy_net::tcp::TcpSocket;
use embedded_io::asynch::Write;
use alloc::string::{String, ToString};

use crate::thermometer::*;
use crate::controller::*;

pub struct Rest<'a>
{
    socket: TcpSocket<'a>,
    buf: [u8; 4096],
    next_stream_head: usize,
}

impl<'a> Rest<'a>
{
    pub fn new(sock: TcpSocket<'a>) -> Self {
        Self { socket: sock, buf: [0; 4096], next_stream_head: 0 }
    }

    pub async fn do_rest_service(&mut self) -> Result<&mut Rest<'a>, String>
    {            
        loop {
            // Read all data until complete parsing.
            let readlen = match self.socket.read(&mut self.buf[self.next_stream_head..]).await {
                Ok(0) => {
                    return Err(String::from("Read EOF"));
                }
                Ok(n) => n,
                Err(e) => {
                    return Err(format!("Read error: {:?}", e));
                }
            };

            log::info!( "do_rest_service(), Receive data: \n{}", from_utf8(&self.buf[..readlen]).unwrap_or("") );

            let request_end = self.next_stream_head + readlen;
            let result = create_rest_response(&self.buf[..request_end]);
            match result.await {
                Ok(opt) => {
                    // complete parsing packet?
                    if let Some(json_response) = opt {
                        // OK, parsing complete and get response.
                        match self.socket.write_all(&json_response.as_bytes()).await {
                            Ok(()) => {
                                // response complete!
                                log::info!("REST Response succeeded: TCPstatus[{}]\n{}", 
                                    get_tcp_state_string(self.socket.state()).as_str(), 
                                    json_response.as_str()
                                );
                            }
                            Err(e) => {
                                return Err(format!("write error: {:?}", e));
                            }
                        };
                        match self.socket.flush().await {
                            Ok(()) => {
                                log::info!("Flush write buffer of socket.");
                                break;
                            }
                            Err(e) => {
                                return Err(format!("flush error {:?}", e));
                            }
                        }
                    }
                    // parsing incomplete. reset head for next read.
                    self.next_stream_head += readlen;
                }
                Err(e) => {
                    return Err(format!("REST internal error: {:?}", e.as_str()));
                }
            };
        }
        
        Ok(self)
    }

    pub async fn accept(&mut self) -> Result<(), String>
    {
        log::info!("Listening on TCP:80..");

        match self.socket.accept(80).await {
            Ok(()) => {
                log::info!("Received connection from {:?}", self.socket.remote_endpoint());
                return Ok(());
            }
            Err(e) => {
                return Err(format!("Listening timeout : {:?}", e));
            }
        }
    }

    pub async fn close(&mut self)
    {
        // close socket and Drop socket instance soon, 
        // FIN packet will NOT send.
        // So we should wait until FIN packet sent, by checking TCP state be "CLOSED" or "TIMEWAIT".
        self.socket.close();
        loop {
            log::info!("Closing socket... TCPstatus[{}]", 
                get_tcp_state_string(self.socket.state()).as_str()
            );
            // Ignore TCP TimeWait sequence because time wait is too long.
            if (self.socket.state() == embassy_net::tcp::State::Closed) ||
               (self.socket.state() == embassy_net::tcp::State::TimeWait) {
                log::info!("Close socket successfuly.");
                break;
            }
            Timer::after(Duration::from_millis(100)).await;
        }
    }

}

async fn create_rest_response(buf: &[u8]) -> Result<Option<String>, String>
{
    let mut headers = [httparse::EMPTY_HEADER; 64];
    let mut request = httparse::Request::new(&mut headers);
    
    let status = request.parse(buf).map_err( |e| { format!("HTTP Header parsing error: {}",e) } )?;
    if status.is_partial() {
        return Ok(None);
    }

    response(&request).map(
        |json| {
            Some(format!("HTTP/1.1 200 OK\r\n\r\n{{{}}}", json))
        }
    )
}

fn response<'a>(request: &httparse::Request<'a, 'a>) -> Result<String, String>
{
    let method = request.method.ok_or("Request method is not found.")?;
    match method {
        "GET"  => { response_get(request) }
        "POST" => Ok(r#"{"error":"Not implemented."}"#.to_string()),
        _      => Ok(r#"{"error":"Invalid method."}"#.to_string()),
    }
}

fn response_get<'a>(request: &httparse::Request<'a, 'a>) -> Result<String, String>
{
    let path = request.path.ok_or("HTTP request path not found.")?;

    match path {
        "/tempareture/heater" => {
            rest_response_tempareture_heater()
        }
        "/tempareture/cpu" => {
            rest_response_tempareture_cpu()
        }
        "/tempareture/all" => {
            rest_response_tempareture_all()
        }
        "/status" => {
            rest_response_status()
        }
        "/details" => {
            rest_response_details()
        }
        _ => { 
            Ok(r#"{"error":"invalid request"}"#.to_string())
        }
    }
}

fn rest_response_tempareture_heater() -> Result<String, String>
{
    let tempareture = heater1_tempareture();
    let json = format!("\"heater_temp\":[{:.2}]", tempareture);
    log::info!("rest_response_tempareture_heater: {}", json.as_str());

    Ok(json)
}

fn rest_response_tempareture_cpu() -> Result<String, String>
{
    let json = format!("\"cpu_temp\":[{:.2}]", cpu_tempareture());
    log::info!("rest_response_tempareture_cpu(): {}", json.as_str());

    Ok(json)
}

fn rest_response_tempareture_all() -> Result<String, String>
{
    let heater_json = rest_response_tempareture_heater()?;
    let cpu_json = rest_response_tempareture_cpu()?;

    let json = format!("{},{}", heater_json, cpu_json);
    log::info!("rest_response_tempareture_all(): {}", json.as_str());

    Ok(json)
}

fn rest_response_status() -> Result<String, String>
{
    let (disp_errcode, disp_message) = match errcode() {
        ErrorCode::None => (0, String::from("")),
        ErrorCode::Heater1OverHeatError {errcode, message} => (errcode, message),
        ErrorCode::Heater1ThermistorDisconnectError {errcode, message} => (errcode, message)
    };

    let json = format!("\"status\":{{\"state\":{},\"err_code\":{},\"message\":{}}}", 
        current_status_string(current_status()),
        disp_errcode,
        disp_message
    );
    log::info!("rest_response_status(): {}", json.as_str());

    Ok(json) 
}

fn rest_response_details() -> Result<String, String>
{
    let temp_json = rest_response_tempareture_all()?;
    let status_json = rest_response_status()?;

    let json = format!("{},{}", temp_json, status_json);
    log::info!("rest_response_details(): {}", json.as_str());

    Ok(json) 
}

fn get_tcp_state_string(state: embassy_net::tcp::State) -> String
{
    match state {
        embassy_net::tcp::State::Closed => String::from("Closed"),
        embassy_net::tcp::State::Listen => String::from("Listen"),
        embassy_net::tcp::State::SynSent => String::from("SynSent"),
        embassy_net::tcp::State::SynReceived => String::from("SynReceived"),
        embassy_net::tcp::State::Established => String::from("Established"),
        embassy_net::tcp::State::FinWait1 => String::from("FinWait1"),
        embassy_net::tcp::State::FinWait2 => String::from("FinWait2"),
        embassy_net::tcp::State::CloseWait => String::from("CloseWait"),
        embassy_net::tcp::State::Closing => String::from("Closing"),
        embassy_net::tcp::State::LastAck => String::from("LastAck"),
        embassy_net::tcp::State::TimeWait => String::from("TimeWait"),
    }
}

fn current_status_string(state: State) -> String
{
    match state {
        State::Initializing => String::from("Initializing"),
        State::Heating => String::from("Heating"),
        State::Saturating => String::from("Saturating"),
        State::Error => String::from("Error"),
    }
}


