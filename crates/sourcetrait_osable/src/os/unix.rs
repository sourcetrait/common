use crate::*;

pub fn mkdtemp<D: AsRef<Path>, S: AsRef<str>>(dir: D, template: S) -> OsableResult<PathBuf> {
    let template = dir.as_ref().join(template.as_ref()).into_utf8()?;
    let template = CString::from_utf8(template)?;
    let ptr = template.into_raw();

    unsafe {
        let result = libc::mkdtemp(ptr);
        let path = CString::from_raw(ptr);

        match result.is_null() {
            true => Err(OsableError::libc_last()),
            false => Ok(PathBuf::from(path.into_utf8()?))
        }
    }
}

pub type UID = u32;
pub type GID = u32;

pub fn uid() -> UID {
    unsafe {
        libc::geteuid()
    }
}

pub fn gid() -> GID {
    unsafe {
        libc::getegid()
    }
}

fn sysconf_size(conf: libc::c_int) -> usize {
    let v = unsafe { libc::sysconf(conf) };
    if v <= 0 { 1024 } else { v as usize }
}

pub fn username_id<S: Into<String>>(username: S) -> OsableResult<Option<UID>> {
    let username = CString::from_utf8(username)?;
    let mut cap = sysconf_size(libc::_SC_GETPW_R_SIZE_MAX);
    let mut buf = vec![0 as libc::c_char; cap];
    let mut pwd: libc::passwd = unsafe { mem::zeroed() };
    let mut result: *mut libc::passwd = std::ptr::null_mut();

    loop {
        let ret = unsafe {
            libc::getpwnam_r(
                username.as_ptr(),
                &mut pwd,
                buf.as_mut_ptr(),
                buf.len(),
                &mut result,
            )
        };
        match (ret, result.is_null()) {
            (0, false) => return Ok(Some(pwd.pw_uid)),
            (0, true) => return Ok(None),
            (libc::ERANGE, _) => {
                cap = cap.checked_mul(2).expect("buffer size overflow");
                buf.resize(cap, 0);
            }
            _ => return OsableError::err_libc(ret),
        }
    }
}

pub fn groupname_id<S: Into<String>>(groupname: S) -> OsableResult<Option<GID>> {
    let groupname = CString::from_utf8(groupname)?;
    let mut cap = sysconf_size(libc::_SC_GETGR_R_SIZE_MAX);
    let mut buf = vec![0 as libc::c_char; cap];
    let mut grp: libc::group = unsafe { mem::zeroed() };
    let mut result: *mut libc::group = std::ptr::null_mut();

    loop {
        let ret = unsafe {
            libc::getgrnam_r(
                groupname.as_ptr(),
                &mut grp,
                buf.as_mut_ptr(),
                buf.len(),
                &mut result,
            )
        };
        match (ret, result.is_null()) {
            (0, false) => return Ok(Some(grp.gr_gid)),
            (0, true) => return Ok(None),
            (libc::ERANGE, _) => {
                cap = cap.checked_mul(2).expect("buffer size overflow");
                buf.resize(cap, 0);
            }
            _ => return OsableError::err_libc(ret),
        }
    }
}

pub fn username(uid: UID) -> OsableResult<Option<String>> {
    let mut cap = sysconf_size(libc::_SC_GETPW_R_SIZE_MAX);
    let mut buf = vec![0 as libc::c_char; cap];
    let mut pwd: libc::passwd = unsafe { mem::zeroed() };
    let mut result: *mut libc::passwd = std::ptr::null_mut();

    loop {
        let ret = unsafe {
            libc::getpwuid_r(
                uid,
                &mut pwd,
                buf.as_mut_ptr(),
                buf.len(),
                &mut result,
            )
        };
        match (ret, result.is_null()) {
            (0, false) => {
                let name = {
                    let cstr = unsafe { CStr::from_ptr(pwd.pw_name) };
                    cstr.into_utf8()?
                };
                return Ok(Some(name));
            }
            (0, true) => return Ok(None),
            (libc::ERANGE, _) => {
                cap = cap.checked_mul(2).expect("buffer size overflow");
                buf.resize(cap, 0);
            }
            _ => return OsableError::err_libc(ret),
        }
    }
}

pub fn groupname(gid: GID) -> OsableResult<Option<String>> {
    let mut cap = sysconf_size(libc::_SC_GETGR_R_SIZE_MAX);
    let mut buf = vec![0 as libc::c_char; cap];
    let mut grp: libc::group = unsafe { mem::zeroed() };
    let mut result: *mut libc::group = std::ptr::null_mut();

    loop {
        let ret = unsafe {
            libc::getgrgid_r(
                gid,
                &mut grp,
                buf.as_mut_ptr(),
                buf.len(),
                &mut result,
            )
        };
        match (ret, result.is_null()) {
            (0, false) => {
                let name = {
                    let cstr = unsafe { CStr::from_ptr(grp.gr_name) };
                    cstr.into_utf8()?
                };
                return Ok(Some(name));
            }
            (0, true) => return Ok(None),
            (libc::ERANGE, _) => {
                cap = cap.checked_mul(2).expect("buffer size overflow");
                buf.resize(cap, 0);
            }
            _ => return OsableError::err_libc(ret),
        }
    }
}

pub fn effective_username() -> OsableResult<String> {
    username(uid())?
        .ok_or_else(|| OsableError::NotFound)
}

pub fn effective_groupname() -> OsableResult<String> {
    groupname(gid())?
        .ok_or_else(|| OsableError::NotFound)
}
