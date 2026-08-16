use std::{fs, path::{Path}};
use sourcetrait_osable as osable;

const OK: &'static str = "ok";

#[test]
fn test_mkdtemp() {
    let actual = osable::unix::mkdtemp(Path::new("/tmp"), "yarr.XXXXXX").expect(OK);
    assert!(actual.is_dir());
    fs::remove_dir(actual).expect(OK);
}

#[test]
fn test_user() {
    let uid = osable::unix::uid();
    let username = osable::unix::effective_username().expect(OK);

    let actual = osable::unix::username_id(&username).expect(OK);
    assert_eq!(actual, Some(uid));
    let actual = osable::unix::username(uid).expect(OK);
    assert_eq!(actual, Some(username));
}

#[test]
fn test_group() {
    let gid = osable::unix::gid();
    let groupname = osable::unix::effective_groupname().expect(OK);

    let actual = osable::unix::groupname_id(&groupname).expect(OK);
    assert_eq!(actual, Some(gid));
    let actual = osable::unix::groupname(gid).expect(OK);
    assert_eq!(actual, Some(groupname));
}
