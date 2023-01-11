use chrono::Utc;
use htmlescape::encode_minimal;
use serde::Deserialize;
use std::env;
use std::fs::File;
use std::io::Write;

#[derive(Debug)]
struct Book {
    id: String,
    title: String,
    author: String,
    publication: String,
    thumbnail_url: String,
    call_number: String,
    library: String,
}

#[derive(Deserialize)]
struct Response {
    success: bool,
    data: Data,
}

#[derive(Deserialize)]
struct Data {
    list: Vec<Datum>,
}

#[derive(Deserialize)]
struct Datum {
    id: i32,
    #[serde(alias = "thumbnailUrl")]
    thumbnail_url: Option<String>,
    #[serde(alias = "titleStatement")]
    title: String,
    author: String,
    publication: String,
    #[serde(alias = "branchVolumes")]
    branch_volumes: Vec<BranchVolume>,
}

#[derive(Deserialize)]
struct BranchVolume {
    #[serde(alias = "name")]
    library: String,
    #[serde(alias = "volume")]
    call_number: String,
}

fn fetch_books(base_url: &str, limit: u8) -> Vec<Book> {
    let url = format!("{}?max={}", base_url, limit);

    let res = reqwest::blocking::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap()
        .get(url)
        .header("User-Agent", "Mozilla/5.0")
        .send()
        .unwrap();
    assert!(res.status().is_success());

    let response = res.json::<Response>().unwrap();
    assert!(response.success);

    let default_thumbnail_url =
        "https://library.ajou.ac.kr/assets/images/ajou/common/default-item-img.png".to_string();
    response
        .data
        .list
        .iter()
        .map(|datum| -> Book {
            Book {
                id: datum.id.to_string(),
                title: datum.title.trim().to_string(),
                author: datum.author.trim().to_string(),
                publication: datum.publication.trim().to_string(),
                thumbnail_url: datum
                    .thumbnail_url
                    .clone()
                    .unwrap_or_else(|| default_thumbnail_url.clone())
                    .trim()
                    .to_string(),
                call_number: datum.branch_volumes[0].call_number.trim().to_string(),
                library: datum.branch_volumes[0].library.trim().to_string(),
            }
        })
        .collect()
}

fn compose_xml(books: &[Book]) -> String {
    let header = format!(
        "<rss version=\"2.0\">\n \
                  <channel>\n \
                  <title>Ajou University Library New Books</title>\n \
                  <link>https://library.ajou.ac.kr/#/new</link>\n \
                  <description>Recently new books</description>\n \
                  <language>ko-kr</language>\n \
                  <lastBuildDate>{}</lastBuildDate>",
        Utc::now().to_rfc2822()
    );

    let footer = "</channel>\n \
                  </rss>";

    let items = books
        .iter()
        .map(|book| -> String {
            let description = format!(
                "{}, {} ({})",
                encode_minimal(&book.author),
                encode_minimal(&book.publication),
                encode_minimal(&book.call_number),
            );
            let link = format!("https://library.ajou.ac.kr/#/search/detail/{}", book.id);
            format!(
                "<item>\n \
                <title>{}</title>\n \
                <link>{}</link>\n \
                <description>{}</description>\n \
                </item>",
                encode_minimal(&book.title),
                encode_minimal(&link),
                encode_minimal(&description),
            )
        })
        .collect::<Vec<String>>()
        .join("\n");

    format!("{}\n{}\n{}", header, items, footer)
}

fn compose_md(books: &[Book]) -> String {
    let header = "# 도서관 단행본 신착";

    let table_header =
        r"| 표지 | 제목 | 저자 | 발행사항 | 청구기호 | 도서관 |\n|----|----|----|----|----|----|";

    let thumbnail_base_url = "https://library.ajou.ac.kr/pyxis-api";
    let items = books
        .iter()
        .map(|book| -> String {
            format!(
                "| ![]({}) | {} | {} | {} | {} | {} |",
                if book.thumbnail_url.starts_with('/') {
                    format!("{}/{}", thumbnail_base_url, book.thumbnail_url)
                } else {
                    book.thumbnail_url.clone()
                },
                book.title,
                book.author,
                book.publication,
                book.call_number,
                book.library,
            )
        })
        .collect::<Vec<String>>()
        .join(r"\n");

    format!(r"{}\n\n{}\n{}", header, table_header, items)
}

fn compose_commit_message(books: &[Book]) -> String {
    let header = "dist: new book(s)".to_string();

    let items = books
        .iter()
        .map(|book| format!("* {}, {}, {}", book.title, book.author, book.publication))
        .collect::<Vec<String>>()
        .join("\n");

    format!("{}\n\n{}", header, items)
}

fn write_last_id(last_id: &str) {
    let current_exe = env::current_exe().unwrap();
    let current_dir = current_exe.parent().unwrap();
    let path = format!("{}/last_id", current_dir.display());
    let mut file = File::create(path).unwrap();
    file.write_all(last_id.to_string().as_bytes()).unwrap();
}

fn main() {
    const BASE_URL: &str = "https://library.ajou.ac.kr/pyxis-api/1/collections/4/search";
    const LIMIT: u8 = 50;

    let args = env::args().collect::<Vec<String>>();
    let last_id = args[1].parse::<String>().unwrap();
    let mode = args[2]
        .parse::<String>()
        .unwrap_or_else(|_| "xml".to_string());

    let books = fetch_books(BASE_URL, LIMIT);
    let latest_id = books
        .iter()
        .collect::<Vec<_>>()
        .first()
        .unwrap()
        .id
        .as_ref();

    if last_id != latest_id {
        match mode.as_str() {
            "xml" => println!("{}", compose_xml(&books)),
            "md" => println!("{}", compose_md(&books)),
            "cm" => println!("{}", compose_commit_message(&books)),
            _ => eprintln!("unknown mode '{}'", mode),
        }

        write_last_id(latest_id);
    } else {
        eprintln!("new books not found")
    }
}
