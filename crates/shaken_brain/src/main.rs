use anyhow::Context;

use tiny_http::{Header, Method, Response, StatusCode};

#[derive(Debug, serde::Deserialize)]
struct Request {
    #[serde(default)]
    min: Option<usize>,

    #[serde(default)]
    max: Option<usize>,

    context: Option<String>,
}

struct Server {
    markov: markov::Markov,
}

fn time_it(label: &str) -> impl Drop + '_ {
    struct TimeIt<'a> {
        start: std::time::Instant,
        label: &'a str,
    }

    impl<'a> Drop for TimeIt<'a> {
        fn drop(&mut self) {
            log::debug!("{} took: {:.2?}", self.label, self.start.elapsed())
        }
    }

    log::debug!("{}", label);
    TimeIt {
        start: std::time::Instant::now(),
        label,
    }
}

impl Server {
    const MIN: usize = 5;
    const MAX: usize = 45;

    fn generate(&self, req: &mut tiny_http::Request) -> anyhow::Result<serde_json::Value> {
        let p: Request = serde_json::from_reader(req.as_reader())?;

        let response = {
            let _t = time_it("generating response");
            self.markov
                .generate(
                    &mut rand::thread_rng(),
                    p.min.unwrap_or(Self::MIN),
                    p.max.unwrap_or(Self::MAX),
                    p.context.as_deref(),
                )
                .expect("generate message")
        };

        Ok(serde_json::json!({
            "status": "ok",
            "data": &response
        }))
    }

    fn handle_req(&self, req: &mut tiny_http::Request) -> anyhow::Result<serde_json::Value> {
        let (method, ep) = (req.method(), req.url());
        log::trace!("{} {}", method, ep);

        match (method, ep) {
            (Method::Get, "/generate") => self.generate(req),
            _ => anyhow::bail!("nope"),
        }
    }

    fn respond(
        req: tiny_http::Request,
        data: serde_json::Value,
        status: impl Into<StatusCode>,
    ) -> std::io::Result<()> {
        let data = serde_json::to_vec(&data).unwrap();

        req.respond(Response::new(
            status.into(),
            vec![Header::from_bytes("Content-Type", "application/json").unwrap()],
            &*data,
            Some(data.len()),
            None,
        ))
    }

    fn host(markov: markov::Markov, address: impl std::net::ToSocketAddrs) -> anyhow::Result<()> {
        const OK: u16 = 200;
        const NOT_OK: u16 = 400;

        let server = tiny_http::Server::http(address).expect("start server");
        log::info!("listening on: {}", server.server_addr());

        let this = Self { markov };
        for mut req in server.incoming_requests() {
            if let Err(err) = match this.handle_req(&mut req) {
                Ok(data) => Self::respond(req, data, OK),
                Err(err) => {
                    let resp = serde_json::json!({
                        "error": err.to_string()
                    });
                    Self::respond(req, resp, NOT_OK)
                }
            } {
                log::error!("cannot respond: {}", err)
            }
        }

        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    alto_logger::init_term_logger().expect("init logger");

    let address = std::env::var("BRAIN_ADDRESS").unwrap_or_else(|_| "localhost:54612".into());

    let brain = std::env::var("BRAIN_FILE")
        .with_context(|| "set `BRAIN_FILE` to the path of the brain db")?;

    let markov = {
        let _t = time_it("loading markov");
        markov::load(brain)?
    };

    log::info!("starting server");
    Server::host(markov, address)
}
