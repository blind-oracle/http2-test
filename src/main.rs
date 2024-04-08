use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;

use http_body_util::Full;
use hyper::body::Bytes;
use hyper::server::conn::{http1, http2};
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio::task::JoinSet;

#[derive(Clone)]
pub struct TokioExecutor;

impl<F> hyper::rt::Executor<F> for TokioExecutor
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    fn execute(&self, fut: F) {
        tokio::task::spawn(fut);
    }
}

async fn hello(_: Request<hyper::body::Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    Ok(Response::new(Full::new(Bytes::from("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"))))
}

async fn client(http2: bool) {
    tokio::time::sleep(3 * std::time::Duration::from_secs(1)).await;

    let mut cli = reqwest::ClientBuilder::new();
    if http2 {
        cli = cli.http2_prior_knowledge();
    }

    let cli = Arc::new(cli.build().unwrap());

    let m = 200;
    let n = 1000;
    let res = Arc::new(Mutex::new(vec![0.0; m]));
    let mut set = JoinSet::new();

    for i in 0..m {
        let cli = cli.clone();
        let res = res.clone();

        set.spawn(async move {
            let start = std::time::Instant::now();
            for _ in 0..n {
                let r = cli.get("http://127.0.0.1:3000").send().await;

                match r {
                    Ok(v) => _ = v.text().await,
                    Err(e) => println!("{:?}", e),
                }
            }

            let sec = start.elapsed().as_secs_f64();
            (res.lock().await)[i] = sec / (n as f64);
        });
    }

    tokio::task::spawn(async move {
        while set.join_next().await.is_some() {}

        println!(
            "avg lat: {}",
            (res.lock().await.iter().sum::<f64>() / (m as f64)) * 1000.0
        );
    });
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let http2 = false;
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    // We create a TcpListener and bind it to 127.0.0.1:3000
    let listener = TcpListener::bind(addr).await?;

    tokio::task::spawn(client(http2));

    // We start a loop to continuously accept incoming connections
    loop {
        let (stream, _) = listener.accept().await?;

        // Use an adapter to access something implementing `tokio::io` traits as if they implement
        // `hyper::rt` IO traits.
        let io = TokioIo::new(stream);

        // Spawn a tokio task to serve multiple connections concurrently
        tokio::task::spawn(async move {
            if http2 {
                if let Err(err) = http2::Builder::new(TokioExecutor)
                    .serve_connection(io, service_fn(hello))
                    .await
                {
                    eprintln!("Error serving connection: {:?}", err);
                }
            } else if let Err(err) = http1::Builder::new()
                .serve_connection(io, service_fn(hello))
                .await
            {
                eprintln!("Error serving connection: {:?}", err);
            }
        });
    }
}
