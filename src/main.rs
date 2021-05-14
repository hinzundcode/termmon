use chrono::prelude::*;
use crate::storage::{Command, SqliteStorage};
use hyper::service::{make_service_fn, service_fn};
use hyper::{Request, Body, Response, StatusCode, Method, header};
use rusqlite::Connection;
use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use url::form_urlencoded;
use base64::decode;
use regex::Regex;

mod storage;

#[derive(Clone)]
struct Server {
	shared: Arc<Shared>,
}

struct Shared {
	state: Mutex<State>,
}

struct State {
	storage: SqliteStorage,
}

impl Server {
	fn new(storage: SqliteStorage) -> Self {
		let shared = Arc::new(Shared {
			state: Mutex::new(State {
				storage
			})
		});
		
		Server { shared }
	}
	
	fn insert_command(&self, command: &mut Command) {
		let state = self.shared.state.lock().unwrap();
		state.storage.insert_command(command);
	}
	
	fn get_latest_commands(&self) -> Vec<Command> {
		let state = self.shared.state.lock().unwrap();
		state.storage.get_latest_commands()
	}
}

async fn handle_request(req: Request<Body>, server: Server) -> Result<Response<Body>, (StatusCode, &'static str)> {
	match (req.method(), req.uri().path()) {
		(&Method::POST, "/commands") => {
			let bytes = hyper::body::to_bytes(req).await
				.map_err(|_| (StatusCode::BAD_REQUEST, "invalid body"))?;
		
			let params: HashMap<String, String> = form_urlencoded::parse(bytes.as_ref())
				.into_owned()
				.collect();
			
			let status: u32 = params.get("status")
				.ok_or((StatusCode::BAD_REQUEST, "status missing"))
				.and_then(|s| s.parse().map_err(|_| (StatusCode::BAD_REQUEST, "status invalid")))?;
				
			let pwd = params.get("pwd").ok_or((StatusCode::BAD_REQUEST, "pwd missing"))?;
			let session_id = params.get("session_id").ok_or((StatusCode::BAD_REQUEST, "session_id missing"))?;
			
			let history = params.get("history").ok_or((StatusCode::BAD_REQUEST, "history missing"))?;
			
			let history = decode(history)
				.map_err(|_| (StatusCode::BAD_REQUEST, "history invalid"))?;
			
			let history = std::str::from_utf8(&history)
				.map_err(|_| (StatusCode::BAD_REQUEST, "history invalid"))?;
			
			let regex = Regex::new(r"\s*(\d+)\s*(.*?)\s*$").unwrap();
			
			let cap = regex.captures(history).ok_or((StatusCode::BAD_REQUEST, "history invalid"))?;
			
			let index: u32 = cap[1].parse().map_err(|_| (StatusCode::BAD_REQUEST, "history invalid"))?;
			
			let command_str = &cap[2];
			
			if command_str != "" {
				let mut command = Command {
					id: 0,
					session_id: session_id.to_lowercase(),
					index,
					command: command_str.to_string(),
					pwd: pwd.to_string(),
					status,
					timestamp: Utc::now(),
				};
				
				server.insert_command(&mut command);
			}
			
			Ok(Response::builder()
				.body(Body::from("")).unwrap())
		},
		(&Method::GET, "/commands") => {
			let commands = server.get_latest_commands();
			
			let history = commands.iter().rev()
				.map(|command| &command.command)
				.fold(String::new(), |a, b| a + b + "\n") + "\n";
			
			Ok(Response::builder()
				.header(header::CONTENT_TYPE, "text/plain")
				.body(Body::from(history)).unwrap())
		}
		_ => {
			Err((StatusCode::BAD_REQUEST, "error"))
		}
	}
}

async fn handle_request_with_error(req: Request<Body>, server: Server) -> Response<Body> {
	handle_request(req, server).await.unwrap_or_else(|(status, string)| {
		Response::builder()
			.status(status)
			.body(Body::from(string)).unwrap()
	})
}

async fn http_server(addr: &SocketAddr, server: Server) {
	let make_svc = make_service_fn(move |_conn| {
		let server = server.clone();
		
		async move {
			Ok::<_, Infallible>(service_fn(move |req| {
				let server = server.clone();
				async move { Ok::<_, Infallible>(handle_request_with_error(req, server).await) }
			}))
		}
	});
	
	let http_server = hyper::Server::bind(addr).serve(make_svc);
	http_server.await.unwrap();
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
	let conn = Connection::open("termmon.db").unwrap();
	let storage = SqliteStorage::new(conn);
	let server = Server::new(storage);
	
	let addr = ([127, 0, 0, 1], 3333).into();
	http_server(&addr, server).await;
}
