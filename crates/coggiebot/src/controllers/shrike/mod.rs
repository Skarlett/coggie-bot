use tokio::io::{BufReader, AsyncBufReadExt};
use seahash::SeaHasher;

use std::collections::HashMap;
use std::hash::BuildHasherDefault;

struct DNSBL(HashSet<String, BuildHasherDefault<SeaHasher>>)


struct Tokens {
    tok: Lexer,
    start: u16,
    end: u16
}

enum Lexer {
    Dot,
    Whitespace,
    Word,
    Uri
}


impl Lexer {
    fn lex<'a>(data: &str, uris: &mut Vec<Token>) {
        data.chars().enumerate().fold((tokens, false), |(tokens, make_uri), (i, x)| {

            // dev.com
            const PATTERN: [Lexer; 3] = [
                Lexer::Word
                Lexer::Dot,
                Lexer::Word
            ];

            let tok = match x {
                 '.' => Lexer::Dot,
                 x.is_whitespace() => Token::Whitespace,
                 _ => Token::Word,
            };

            if tok == s.last() {
                s.last_mut().unwrap().end += 1;
                continue
            }


            let top = tokens.take(3).collect::<[Lexer; 3]>

            if top == PATTERN {
                let start = tokens.first().start;
                let end = tokens.last().end;

                tokens.push(Token { tok: Token::Uri, start, end });
            }
        })

        // [word] [dot] [word]
    }
}



impl DNSBL {
    fn check(&self, uri: &str) -> bool {
        if uri.contains("://") {
            let top = uri.split_once("://").unwrap().1;
            match top.split_once("/") {
                Some((domain, _)) => self.0.contains(domain),
                None => top
            }
            
        }


        self.0.contains(uri)
    }
}

pub async fn dnsbl_add(dnsbl: &mut DNSBL, fp: &Path, ) -> Result<PlaySource, DxError>
{
    let mut set = HashSet::with_hasher(BuildHasherDefault::<SeaHasher>::default());

    let lines = tokio::fs::File::open(fp)
        .await?
        .lines(&mut set);

    while let Ok(Some(line)) = lines.next_line().await {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }

        let (_, domain) = line.split_once(' ');
        map.insert(domain.to_string());
    }
}


// #[tracing::instrument]
pub async fn scan_attachments(http: Arc<Http>, msg: &Message) -> Result<PlaySource, DxError>
{
    let tmpdir = tempfile::tempdir()?;

    tracing::info!("RUNNING: clamav --portable -p {} {}", &tmpdir.path().display(), uri);
    let child = tokio::process::Command::new("clamscan")
        .current_dir(dx.cache.as_ref().unwrap())
        .arg("-p").arg(&tmpdir.path())
        .arg(uri)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start child process");

    let out = child.wait_with_output().await?;
    let mut error_buf = String::new();

    tracing::info!("deemix exit code: {}", out.status);
    tracing::warn!("deemix stderr: {}", String::from_utf8_lossy(&out.stderr[..]));
    tracing::debug!("deemix stdout: {}", String::from_utf8_lossy(&out.stdout[..]));

    let paths = process_dir(&tmpdir.path(), &dx.cache.as_ref().unwrap().join("music") ).await?;
    // tokio::fs::remove_dir_all(&tmpdir).await?;


    return Ok(PlaySource::FileSystem {
        errlog: error_buf,
        ok_paths: paths,
    });
}
