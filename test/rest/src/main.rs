use actix_web::{get, HttpServer, Responder, App};
use actix_web::http::header;
use actix_cors::Cors;

#[get("/details")]
async fn details() -> impl Responder
{
    let cpu_temp = 25.0;
    let heater_temp = 30.0;

    let cputemp_json = format!("\"cpu_temp\":[{:.2}]", cpu_temp);
    let heatertemp_json = format!("\"heater_temp\":[{:.2}]", heater_temp);

    format!("{{{}, {}}}", cputemp_json, heatertemp_json)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(
        || {
            let cors = Cors::default()
                .allowed_origin_fn(|origin, _req_head| {
                    true
                })
                .allowed_methods(vec!["GET"])
                .allowed_headers(vec![header::AUTHORIZATION, header::ACCEPT])
                .allowed_header(header::CONTENT_TYPE)
                .supports_credentials()
                .max_age(3600);

            App::new()
            .wrap(cors)
            .service(details)
        }
    )
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
