use slab::Slab;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::net::{SocketAddr};
use hyper:: {Response, Server, Body, Request, Method, Error, StatusCode};
use hyper::service::service_fn;
use futures::{future, Future};
use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    static ref INDEX_PATH: Regex = Regex::new("^/(index\\.html?)?$").unwrap();// matches / , /index.htm , /index.html
    // ^ means must be a string beginning; $ means must be a str ending ; => expect exact matching
    // () implies there must be a group. A group is a unit
    // ? means previous char is optional, and hence second ? means matching just /
    // . fits any symbol, but we need an actual dot so put \. But single \ treated as escape, so need \\.
    static ref USER_PATH: Regex = Regex::new("^/user/((?P<user_id>\\d+?)/?)?$").unwrap();// /user/ , /user/<id> where <id> means group of digits, /user/<id>/
    // ?P sets name of group(regex::Captures accesses the group)
    // \\d matches any digit, \d+ matches any one or more digits, * would be used for 0 repetitions
    static ref USERS_PATH: Regex = Regex::new("^/users/?$").unwrap();// /users , /users/
}

fn main() {
    let addr: SocketAddr = ([127, 0, 0, 1], 8080).into();
    // Builder provides methods to tweak params of server(eg HTTP1 or 2 or both)
    let builder = Server::bind(&addr);
    // serve is using builder to attach a service for handling incoming HTTP requests
    // we pass a fn to builder which generates a Service instance
    /*
    let server = builder.serve(|| {
        // service_fn_ok turns function into a service (handler)
        service_fn_ok( |_| {
            // this fn takes a request(currently _) and gives back a responce
            // give same response to every request 
            Response::new(Body::from("Almost there"))
        })
    }); */
    let udb = Arc::new(Mutex::new(Slab::new()));
    // moving udb here so won't be able to use again
    let server = builder.serve(move || {
        let user_db = Arc::clone(&udb);
        // clone here because the closure will be called multiple times
        service_fn(move |req|
        microservice_handler(req, &user_db))});
    
    let server = server.map_err(drop); //for this need Future
    hyper::rt::run(server);
}

fn microservice_handler(req: Request<Body>, user_db: &UserDB) -> impl Future<Item = Response<Body>, Error=Error> {
    let response = { 
        match (req.method(), req.uri().path()) {
        (&Method::GET, "/") => {
            //body is an html string
            Response::new(INDEX.into())
        },
        (method, path) if path.starts_with(USER_PATH) => { 
            let user_id = path.trim_left_matches(USER_PATH)
                                    .parse::<UserId>()
                                    .ok() // convert Result into an Option
                                    .map(|x| x as usize);
            let mut users = user_db.lock().unwrap();
         },
         (&Method::DELETE, Some(id)) => {
            if users.contains(id) {
                users.remove(id);
                response_with_code(StatusCode::OK)
            } else {
                response_with_code(StatusCode::NOT_FOUND)
            }
         },
         (&Method::PUT, Some(id)) => {
             if let Some(user) = users.get_mut(id){
                 *user = UserData;
                 response_with_code(StatusCode::OK)
             } else {
                 response_with_code(StatusCode::NOT_FOUND)
             }
        },
         (&Method::GET, Some(id)) => {
             if let Some(data) = users.get(id) {
                 Response::new(data.to_string().into())
             } else {
                 response_with_code(StatusCode::NOT_FOUND)
             }
         },

         (&Method::POST, None) => {
             let id = users.insert(UserData);
             Response::new(id.to_string().into())
         },

         (&Method::POST, Some(_)) => {response_with_code(StatusCode::BAD_REQUEST)},

        _ => {response_with_code(StatusCode::METHOD_NOT_ALLOWED)}
        }
    };
    future::ok(response)
}

fn response_with_code(status_code: StatusCode) -> Response<Body> {
    Response::builder()
        .status(status_code)
        .body(Body::empty())
        .unwrap()
}

// # is multiline string
const INDEX: &'static str = r#"
<!doctype html>
<html>
    <head>
        <title>Rust Microservices</title>
    </head>
    <body>
        <h3>Rust Microservices</h3>
    </body>
</html>
"#;

type UserId = u64;
#[derive(Debug)]
struct UserData;
//Display auto derives ToString
impl fmt::Display for UserData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        //usually use serde here
        f.write_str("{}")
    }
}
// Slab is an allocator that can store and remove any value identified by an ordered number, and also reuses the slots
// similar to Vec, but doesn't resize if you remove, but wil automatically reuse
// Arc lets us provide references in a shared state
type UserDB = Arc<Mutex<Slab<UserData>>>;