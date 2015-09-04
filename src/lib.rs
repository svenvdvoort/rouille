extern crate num_cpus;
extern crate rustc_serialize;
extern crate threadpool;
extern crate time;
extern crate tiny_http;
extern crate url;

pub use assets::match_assets;
pub use log::LogEntry;

use std::net::SocketAddr;
use std::net::ToSocketAddrs;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::thread;
use threadpool::ThreadPool;

pub mod input;

mod assets;
mod log;
mod router;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RouteError {
    /// Couldn't find a way to handle this request.
    NoRouteFound,

    WrongInput,
}

/// Starts a server and uses the given requests handler.
pub fn start_server<A, F>(addr: A, handler: F) -> !
                          where A: ToSocketAddrs,
                                F: Send + Sync + 'static + Fn(&Request) -> Response
{
    // FIXME: directly pass `addr` to the `TcpListener`
    let port = addr.to_socket_addrs().unwrap().next().unwrap().port();
    let server = tiny_http::ServerBuilder::new().with_port(port).build().unwrap();

    let handler = Arc::new(handler);
    let pool = ThreadPool::new(num_cpus::get());

    loop {
        let request = server.recv().unwrap();

        let request = Request {
            url: request.url().to_owned(),
            request: Mutex::new(Some(request)),
            data: Mutex::new(None),
        };

        let handler = handler.clone();

        pool.execute(move || {
            let response = (*handler)(&request);
            request.request.lock().unwrap().take().unwrap().respond(response.response);
        });
    }
}

/// Represents a request made by the client.
pub struct Request {
    url: String,
    request: Mutex<Option<tiny_http::Request>>,     // TODO: when Mutex gets "into_inner", remove the Option
    data: Mutex<Option<Vec<u8>>>,
}

impl Request {
    /// Returns the URL requested by the client. It is not decoded and thus can contain `%20` or
    /// other characters.
    #[inline]
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Returns the value of a header of the request.
    #[inline]
    pub fn header(&self, key: &str) -> Option<String> {
        self.request.lock().unwrap().as_ref().unwrap().headers().iter()
                    .find(|h| h.field.as_str() == key).map(|h| h.value.as_str().to_owned())
    }

    /// Returns the data of the request.
    pub fn data(&self) -> Vec<u8> {
        let mut data = self.data.lock().unwrap();

        if let Some(ref mut data) = *data {
            return data.clone();
        }

        let mut read = Vec::new();
        let _ = self.request.lock().unwrap().as_mut().unwrap()
                            .as_reader().read_to_end(&mut read);
        let read_clone = read.clone();
        *data = Some(read);
        read_clone
    }
}

/// Contains a prototype of a response. The response is only sent when you call `Request::respond`.
pub struct Response {
    response: tiny_http::ResponseBox,
}

impl Response {
    /// Builds a default response to handle the given route error.
    #[inline]
    pub fn from_error(err: &RouteError) -> Response {
        let response = match err {
            &RouteError::NoRouteFound => tiny_http::Response::empty(404),
            &RouteError::WrongInput => tiny_http::Response::empty(400),
        };

        Response { response: response.boxed() }
    }

    /// Builds a `Response` that redirects the user to another URL.
    #[inline]
    pub fn redirect(target: &str) -> Response {
        let response = tiny_http::Response::empty(303);
        // TODO: slow \|/
        let response = response.with_header(tiny_http::Header::from_str(&format!("Location: {}", target)).unwrap());

        Response { response: response.boxed() }
    }

    /// Builds a `Response` that outputs HTML.
    #[inline]
    pub fn html(content: &[u8]) -> Response {
        let response = tiny_http::Response::from_data(content);
        // TODO: slow \|/
        let response = response.with_header(tiny_http::Header::from_str("Content-Type: text/html; charset=utf8").unwrap());

        Response { response: response.boxed() }
    }

    /// Builds a `Response` that outputs JSON.
    #[inline]
    pub fn json<T>(content: &T) -> Response where T: rustc_serialize::Encodable {
        let data = rustc_serialize::json::encode(content).unwrap();
        let response = tiny_http::Response::from_string(data);
        // TODO: slow \|/
        let response = response.with_header(tiny_http::Header::from_str("Content-Type: application/json").unwrap());

        Response { response: response.boxed() }
    }
}
