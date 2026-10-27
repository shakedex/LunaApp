use std::io::Cursor;

use luna_formats::{ClipFiles, Container};
use luna_worker::protocol::{MAX_MESSAGE, read_message, write_message};
use luna_worker::{ClipJob, ClipResult, Request};

fn job() -> Request {
    Request::Job(ClipJob {
        id: 7,
        files: ClipFiles {
            container: Container::Mxf,
            main: "D:/Shoot/A001/A001C001.mxf".into(),
            segments: vec![],
            sidecars: vec![],
        },
        strip: 3,
        thumb_width: 640,
        colour: Default::default(),
    })
}

#[test]
fn messages_round_trip() {
    let mut buf = Vec::new();
    write_message(&mut buf, &job()).unwrap();
    write_message(&mut buf, &Request::Shutdown).unwrap();
    let mut r = Cursor::new(buf);
    assert_eq!(read_message::<Request>(&mut r).unwrap(), Some(job()));
    assert_eq!(
        read_message::<Request>(&mut r).unwrap(),
        Some(Request::Shutdown)
    );
    assert_eq!(read_message::<Request>(&mut r).unwrap(), None, "clean EOF");
}

#[test]
fn results_carry_images_and_reasons() {
    let result = ClipResult {
        id: 7,
        meta: None,
        codec: None,
        bytes: 42,
        thumbnails: vec![Ok(vec![0xFF, 0xD8, 0xFF]), Err("No decoder".into())],
        warnings: vec!["w".into()],
        colour: None,
    };
    let mut buf = Vec::new();
    write_message(&mut buf, &result).unwrap();
    let back: ClipResult = read_message(&mut Cursor::new(buf)).unwrap().unwrap();
    assert_eq!(back, result);
}

#[test]
fn truncated_message_is_an_error_not_eof() {
    let mut buf = Vec::new();
    write_message(&mut buf, &job()).unwrap();
    buf.truncate(buf.len() - 3);
    assert!(read_message::<Request>(&mut Cursor::new(buf)).is_err());
}

#[test]
fn absurd_length_is_rejected_without_allocating() {
    let buf = (MAX_MESSAGE as u32 + 1).to_le_bytes().to_vec();
    assert!(read_message::<Request>(&mut Cursor::new(buf)).is_err());
}
