use core::str::from_utf8;
use defmt::*;

use embassy_net::tcp::TcpSocket;
use embedded_io::asynch::Write;
use alloc::string::{String, ToString};


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
                        match self.socket.write(&json_response.as_bytes()).await {
                            Ok(size) => {
                                // response complete!
                                log::info!("REST Response succeeded: \n{}", json_response.as_str());
                                break;
                            }
                            Err(e) => {
                                return Err(format!("write error: {:?}", e));
                            }
                        };
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
}

async fn create_rest_response(buf: &[u8]) -> Result<Option<String>, String>
{
    let mut headers = [httparse::EMPTY_HEADER; 64];
    let mut request = httparse::Request::new(&mut headers);
    
    let status = request.parse(buf).map_err( |e| { format!("HTTP Header parsing error: {}",e) } )?;
    if status.is_partial() {
        return Ok(None);
    }

    match response(&request) {
        Ok(json) => {
            Ok(Some(format!("HTTP/1.1 200 OK\r\n\r\n{}", json)))
        }
        Err(e) => Err(e)
    }
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
        _ => { 
            Ok(r#"{"error":"invalid request"}"#.to_string())
        }
    }
}

fn rest_response_tempareture_heater() -> Result<String, String>
{
    let json = format!("{{\"heater_temp\":[{}]}}", 25.0);
    log::info!("rest_response_tempareture_heater: {}", json.as_str());

    Ok(json)
}

fn rest_response_tempareture_cpu() -> Result<String, String>
{
    let json = format!("{{\"cpu_temp\":[{}]}}", 60.0);
    log::info!("rest_response_tempareture_cpu(): {}", json.as_str());

    Ok(json)
}



