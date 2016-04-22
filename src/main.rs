#[macro_use] extern crate log;
extern crate env_logger;
extern crate rustc_serialize;
extern crate mustache;
extern crate getopts;
extern crate mime;
extern crate hyper;
extern crate pulldown_cmark;

use std::io::{self, Read, Write};
use std::net::Ipv4Addr;
use std::fs::File;
use std::fs::read_dir;
use std::path::Path;
use hyper::Get;
use hyper::header::{ContentLength, ContentType};
use hyper::server::{Server, Handler, Request, Response, Fresh};
use hyper::uri::RequestUri::AbsolutePath;

use compat::PathExt;

mod compat;
mod cmark;

macro_rules! try_return {
    ($e:expr) => {{
        match $e {
            Ok(v) => v,
            Err(e) => { error!("Error: {}", e); return; }
        }
    }}
}

#[derive(RustcEncodable)]
struct Ctx {
    content: String,
    title: String,
}

#[derive(Clone)]
struct RustKrServer {
    port: u16,
    doc_dir: String,
    static_dir: String,
    template: mustache::Template,
}

impl Handler for RustKrServer {
    fn handle<'a, 'k>(&'a self, req: Request<'a, 'k>, res: Response<'a, Fresh>) {
        if req.method == Get {
            let uri = req.uri.clone();
            if let AbsolutePath(ref uri) = uri {
                macro_rules! handlers {
                    (
                        $(
                            ($path:expr, $handler:ident),
                        )+
                    ) => (
                        {
                            $(
                                if uri.starts_with($path) {
                                    let remaining = &uri[$path.len()..];
                                    self.$handler(remaining, req, res);
                                    return;
                                }
                            )+
                        }
                    )
                }

                handlers!(
                    ("/static/", handle_static_file),
                    ("/pages/", handle_page),
                    ("/", handle_index_page),
                );
            }
        }

        // fallthrough
        self.show_bad_request(req, res);
        return;
    }
}

impl RustKrServer {
    fn is_bad_title(&self, title: &str) -> bool {
        for c in title.chars() {
            match c {
                'A'...'Z' | 'a'...'z' | '0'...'9' | '_' | '-' => continue,
                _ => return true,
            }
        }

        false
    }

    fn read_page(&self, title: &str) -> io::Result<String> {
        let path = format!("{}/{}.md", self.doc_dir, title);
        let path = Path::new(&path);
        let mut f = try!(File::open(&path));
        let mut text = String::new();
        try!(f.read_to_string(&mut text));
        let md = cmark::to_html(&text);
        Ok(format!("{}", md))
    }

    pub fn list_pages(&self) -> String {
        let dir = Path::new(&self.doc_dir);
        if !dir.exists() {
            return "No pages found".to_string();
        }

        let files = match read_dir(&dir) {
            Ok(files) => files,
            Err(_) => return "Error during reading dir".to_string(),
        };
        let mut pages = vec![];
        for file in files {
            let file = match file {
                Ok(f) => f.path(),
                _ => continue,
            };
            if file.is_dir() {
                continue;
            }
            match file.as_os_str().to_str() {
                None => continue,
                Some(s) => {
                    if s.ends_with(".md") {
                        let pagename = file.file_stem();
                        match pagename {
                            None => continue,
                            Some(pagename) => {
                                let pagename = match pagename.to_str() {
                                    Some(p) => p,
                                    None => continue,
                                };
                                if self.is_bad_title(pagename) {
                                    continue;
                                }
                                pages.push(pagename.to_string());
                            }
                        }
                    }
                }
            }
        }

        pages.sort();

        if pages.len() > 0 {
            let mut ret = "<ul>\n".to_string();
            for page in pages.iter() {
                ret = ret + &format!(r#"<li><a href="/pages/{}">{}</a></li>"#, *page, *page);
            }
            ret = ret + "</ul>";
            ret
        } else {
            "No pages found".to_string()
        }
    }

    fn show_not_found(&self, req: Request, res: Response) {
        let ctx = Ctx {
            title: "Not Found".to_string(),
            content: "헐".to_string(),
        };
        self.show_template(req, res, &ctx);
    }

    fn show_bad_request(&self, req: Request, res: Response) {
        let ctx = Ctx {
            title: "Bad request".to_string(),
            content: "헐".to_string(),
        };
        self.show_template(req, res, &ctx);
    }

    fn show_template(&self, _: Request, mut res: Response, ctx: &Ctx) {
        let mut output = vec![];
        match self.template.render(&mut output, ctx) {
            Ok(()) => {}
            Err(_) => return,
        }

        {
            let headers = res.headers_mut();

            headers.set(ContentLength(output.len() as u64));
            let content_type = mime::Mime(mime::TopLevel::Text, mime::SubLevel::Html, vec![]);
            headers.set(ContentType(content_type));
        }

        let mut res = try_return!(res.start());
        try_return!(res.write_all(&output));
        try_return!(res.end());
    }

    fn handle_index_page(&self, remaining: &str, req: Request, res: Response) {
        if remaining.len() > 0 {
            self.show_not_found(req, res);
            return;
        }
        self.handle_page("index", req, res);
    }

    fn handle_page(&self, title: &str, req: Request, res: Response) {
        debug!("handle page: {}", title);
        let (title, content) = match title {
            "_pages" => ("모든 문서", self.list_pages()),
            _ => {
                let content = self.read_page(title);
                match content.ok() {
                    Some(content) => (title, content),
                    None => {
                        return self.show_not_found(req, res);
                    }
                }
            }
        };
        let ctx = Ctx {
            title: title.to_string(),
            content: content,
        };
        self.show_template(req, res, &ctx);
    }

    fn handle_static_file(&self, loc: &str, req: Request, mut res: Response) {
        let path = format!("{}/{}", self.static_dir, loc);
        let path = Path::new(&path);
        if !path.exists() {
            self.show_not_found(req, res);
            return;
        }
        let mut f = try_return!(File::open(&path));
        let mut output = Vec::new();
        try_return!(f.read_to_end(&mut output));

        {
            use mime::{Mime, SubLevel, TopLevel};

            let headers = res.headers_mut();

            headers.set(ContentLength(output.len() as u64));

            let (toplevel, sublevel) = match path.extension().and_then(|ext| ext.to_str()) {
                Some("css") => (TopLevel::Text, SubLevel::Css),
                Some("svg") => (TopLevel::Image, SubLevel::Ext("svg+xml".to_owned())),
                _ => (TopLevel::Text, SubLevel::Plain),
            };
            let params = vec![(mime::Attr::Charset, mime::Value::Utf8)];
            headers.set(ContentType(Mime(toplevel, sublevel, params)));
        }

        let mut res = try_return!(res.start());
        try_return!(res.write_all(&output));
        try_return!(res.end());
    }
}

fn main() {
    env_logger::init().unwrap();

    let mut opts = getopts::Options::new();
    opts.optopt("p", "port", "server port number", "PORT");
    opts.optopt("", "docs", "path of markdown docs", "PATH");
    opts.optopt("", "static", "path of static files", "PATH");
    opts.optopt("", "template", "template path", "PATH");
    opts.optopt("", "num-threads", "size of thread pool", "NUM");

    let args: Vec<_> = std::env::args().skip(1).collect();
    let matches = opts.parse(&args).ok().expect("Bad opts");
    let port: u16 = matches.opt_str("port").unwrap_or("8000".to_string()).parse().unwrap();
    let doc_dir = matches.opt_str("docs").unwrap_or("docs".to_string());
    let static_dir = matches.opt_str("static").unwrap_or("static".to_string());
    let template_path = matches.opt_str("template")
                               .unwrap_or("templates/default.mustache".to_string());
    let num_threads = matches.opt_str("num-threads").unwrap_or("10".to_string()).parse().unwrap();

    debug!("port: {} / doc_dir: {} / static_dir: {} / template_path: {} / num_threads: {}",
           port, doc_dir, static_dir, template_path, num_threads);

    let template = mustache::compile_path(Path::new(&template_path)).unwrap();

    let rskr = RustKrServer {
        port: port,
        doc_dir: doc_dir,
        static_dir: static_dir,
        template: template,
    };

    let addr = (Ipv4Addr::new(127, 0, 0, 1), port);
    let server = Server::http(addr).unwrap();
    server.handle_threads(rskr, num_threads).unwrap();
    debug!("listening...");
}
