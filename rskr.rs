extern mod extra;
extern mod http;

use std::rt::io::net::ip::{SocketAddr, Ipv4Addr};
use std::rt::io::Writer;
use extra::time;

use http::server::{Config, Server, ServerUtil, Request, ResponseWriter};
use http::server::request::AbsolutePath;
use http::headers::content_type::MediaType;

#[deriving(Clone)]
struct RustKrServer;

impl Server for RustKrServer {
    fn get_config(&self) -> Config {
        Config {
            bind_address: SocketAddr {
                ip: Ipv4Addr(127, 0, 0, 1),
                port: 8001,
            }
        }
    }

    fn handle_request(&self, r: &Request, w: &mut ResponseWriter) {
        let content = match r.request_uri {
            AbsolutePath(ref url) => {
                // remove '/'
                let url = url.slice_from(1);
                self.read_page(url)
            },
            _ => {
                ~"tekitou"
            }
        };

        let header = r#"<!doctype html>
<html>
<head>
<meta http-equiv="Content-Type" content="text/html; charset=utf-8" />
<title>한국 러스트 사용자 그룹</title>
</head>
<body>"#;
        let footer = "</body> </html>";

        let output = header + content + footer;
        let output_b = output.as_bytes();

        w.headers.date = Some(time::now_utc());
        w.headers.content_length = Some(output_b.len());
        w.headers.content_type = Some(MediaType {
            type_: ~"text",
            subtype: ~"html",
            parameters: ~[(~"charset", ~"UTF-8")]
        });
        w.headers.server = Some(~"Example");

        w.write(output_b);
    }
}

impl RustKrServer {
    fn read_page(&self, title: &str) -> ~str {
        format!("read_page: title: {:s}", title)
    }
}

fn main() {
    RustKrServer.serve_forever();
}
