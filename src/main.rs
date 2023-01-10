use actix_files::NamedFile;
use actix_web::{HttpRequest, Result};
use std::path::PathBuf;

async fn index_route(_req: HttpRequest) -> Result<NamedFile> {
    Ok(NamedFile::open("static/index.html")?)
}

async fn static_route(req: HttpRequest) -> Result<NamedFile> {
    let mut path: PathBuf = "static".into();
    let fname: PathBuf = req.match_info().query("filename").parse().unwrap();
    path.push(fname);
    Ok(NamedFile::open(path)?)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    use actix_web::{web, App, HttpServer};

    const PORT: u16 = 8080;

    println!("Start HTTP server: http://127.0.0.1:{}", PORT);
    HttpServer::new(|| App::new()
        .route("/", web::get().to(index_route))
        .route("/static/{filename:.*}", web::get().to(static_route))
    )
    .bind(("127.0.0.1", PORT))?
    .run()
    .await
}