extern mod extra;
extern mod http;

use std::rt::io::net::ip::{SocketAddr, Ipv4Addr};
use std::rt::io::Writer;
use extra::time;

use http::server::{Config, Server, ServerUtil, Request, ResponseWriter};
use http::server::request::AbsolutePath;
use http::headers::content_type::MediaType;

// assumes utf-8
pub trait PercentDecoder {
    fn decode_percent(&self) -> ~str;
}

impl<'self> PercentDecoder for &'self str {
    fn decode_percent(&self) -> ~str {
        fn hex_to_u8(h: &Ascii) -> u8 {
            let h = h.to_byte();
            match h {
                0x30..0x39 => h - 0x30, // '0'..'9'
                0x41..0x46 => h - 0x41 + 10, // 'A'..'F'
                0x61..0x66 => h - 0x61 + 10, // 'a'..'f'
                _ => fail!("not a hex value")
            }
        }

        let mut buf: ~[u8] = ~[];
        let mut it = self.to_ascii().iter();
        loop {
            let c = it.next();
            match c {
                None => break,
                Some(c) => {
                    let c = c.to_byte();
                    if c == 0x25 {
                        let c1 = hex_to_u8(it.next().unwrap());
                        let c2 = hex_to_u8(it.next().unwrap());
                        let cc = c1 * 16 + c2;
                        buf.push(cc);
                    } else {
                        buf.push(c);
                    }
                }
            }
        }

        std::str::from_utf8_owned(buf)
    }
}

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
                let title = url.slice_from(1);
                let title = title.decode_percent();
                self.read_page(title)
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

#[cfg(test)]
mod test {
    use super::PercentDecoder;

    fn compare(input: &str, output: &str) {
        assert_eq!(input.decode_percent(), output.to_owned());
    }
    #[test]
    fn decode_percent() {
        compare("abc", "abc");
        compare("a%20bc", "a bc");
        compare("a%2Fbc", "a/bc");
        compare("%EA%B0%80%EB%82%98%EB%8B%A4", "가나다");
    }
}
