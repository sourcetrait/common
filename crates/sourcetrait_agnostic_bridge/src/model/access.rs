use crate::*;

pub type StringSID = String;
pub type StrSID = str;
pub type UnixID = u32;
pub type UID = UnixID;
pub type GID = UnixID;
pub type UnixFileMode = u32;

/// Used for cross-platform lookups, so it only supports UTF-8
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AccessKey {
    Name(TwoString),
    QualifiedName(TwoString, TwoString),
    UnixID(UnixID),
    WindowsSID(StringSID),
}


/// Used for cross-platform lookups, so it only supports UTF-8
#[derive(Debug, Clone, Copy)]
pub enum AccessKeyRef<'a> {
    Name(TwoStr<'a>),
    QualifiedName(TwoStr<'a>, TwoStr<'a>),
    UnixID(UnixID),
    WindowsSID(&'a StrSID),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AccessId {
    UnixID(UnixID),
    WindowsSID(StringSID),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessIdRef<'a> {
    UnixID(UnixID),
    WindowsSID(&'a StrSID),
}

impl<'a> AccessIdRef<'a> {
    pub fn unix_id(&self) -> BridgeResult<UnixID> {
        match self {
            Self::UnixID(id) => Ok(*id),
            _ => Err(BridgeError::Expected { noun: BridgeErr::UnixID }),
        }
    }
    
    pub fn windows_sid(&self) -> BridgeResult<&StrSID> {
        match self {
            Self::WindowsSID(sid) => Ok(sid),
            _ => Err(BridgeError::Expected { noun: BridgeErr::WindowsSID }),
        }
    }
}

impl<'a> Display for AccessIdRef<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnixID(id) => write!(f, "{}", id),
            Self::WindowsSID(sid) => write!(f, "{}", sid),
        }
    }
}

impl<'a> AccessKeyRef<'a> {
    pub fn name<S: Into<TwoStr<'a>>>(name: S) -> Self {
        Self::Name(name.into())
    }
    
    pub fn qualified_name<S1: Into<TwoStr<'a>>, S2: Into<TwoStr<'a>>>(domain: S1, name: S2) -> Self {
        Self::QualifiedName(domain.into(), name.into())
    }
    
    pub fn windows_sid<S: Into<&'a StrSID>>(sid: S) -> Self {
        Self::WindowsSID(sid.into())
    }
    
    pub fn unix_id(id: UnixID) -> Self {
        Self::UnixID(id)
    }
}

pub trait AsAccess<'a> {
    fn as_key_ref(&'a self) -> AccessKeyRef<'a>;
}

impl<'a> AsAccess<'a> for AccessKey {
    fn as_key_ref(&'a self) -> AccessKeyRef<'a> {
        match self {
            AccessKey::Name(v) => AccessKeyRef::Name(v.as_two_str()),
            AccessKey::QualifiedName(v1, v2) => AccessKeyRef::QualifiedName(v1.as_two_str(), v2.as_two_str()),
            AccessKey::UnixID(v) => AccessKeyRef::UnixID(*v),
            AccessKey::WindowsSID(v) => AccessKeyRef::WindowsSID(v.as_str()),
        }
    }
}

impl From<AccessKeyRef<'_>> for AccessKey {
    fn from(value: AccessKeyRef<'_>) -> Self {
        match value {
            AccessKeyRef::Name(v) => AccessKey::Name(v.into()),
            AccessKeyRef::QualifiedName(v1, v2) => AccessKey::QualifiedName(v1.into(), v2.into()),
            AccessKeyRef::UnixID(v) => AccessKey::UnixID(v),
            AccessKeyRef::WindowsSID(v) => AccessKey::WindowsSID(v.into()),
        }
    }
}

impl<'a> From<&'a AccessKey> for AccessKeyRef<'a> {
    fn from(value: &'a AccessKey) -> Self {
        match value {
            AccessKey::Name(v) => AccessKeyRef::Name(v.as_two_str()),
            AccessKey::QualifiedName(v1, v2) => AccessKeyRef::QualifiedName(v1.as_two_str(), v2.as_two_str()),
            AccessKey::UnixID(v) => AccessKeyRef::UnixID(*v),
            AccessKey::WindowsSID(v) => AccessKeyRef::WindowsSID(v.as_str()),
        }
    }
}

// SAFETY: this is safe for hash check calls. don't store results.
// we're lying about the lifetime, which is valid so long as we don't store it.
impl<'a> Borrow<AccessKeyRef<'a>> for AccessKey {
    fn borrow(&self) -> &AccessKeyRef<'a> {
        unsafe {
            match self {
                AccessKey::Name(s) => 
                    std::mem::transmute::<&AccessKeyRef<'_>, &AccessKeyRef<'a>>(
                        &AccessKeyRef::Name(s.as_two_str())
                    ),
                AccessKey::QualifiedName(s1, s2) => 
                    std::mem::transmute::<&AccessKeyRef<'_>, &AccessKeyRef<'a>>(
                        &AccessKeyRef::QualifiedName(s1.as_two_str(), s2.as_two_str())
                    ),
                AccessKey::UnixID(id) => 
                    std::mem::transmute::<&AccessKeyRef<'_>, &AccessKeyRef<'a>>(
                        &AccessKeyRef::UnixID(*id)
                    ),
                AccessKey::WindowsSID(sid) => 
                    std::mem::transmute::<&AccessKeyRef<'_>, &AccessKeyRef<'a>>(
                        &AccessKeyRef::WindowsSID(sid.as_ref())
                    ),
            }
        }
    }
}

impl<'a> Hash for AccessKeyRef<'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Self::Name(s) => {
                0u8.hash(state);
                s.hash(state);
            },
            Self::QualifiedName(s1, s2) => {
                1u8.hash(state);
                s1.hash(state);
                s2.hash(state);
            },
            Self::UnixID(id) => {
                2u8.hash(state);
                id.hash(state);
            },
            Self::WindowsSID(sid) => {
                3u8.hash(state);
                sid.hash(state);
            },
        }
    }
}

impl<'a> PartialEq for AccessKeyRef<'a> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Name(a), Self::Name(b)) => a == b,
            (Self::QualifiedName(a1, a2), Self::QualifiedName(b1, b2)) => a1 == b1 && a2 == b2,
            (Self::UnixID(a), Self::UnixID(b)) => a == b,
            (Self::WindowsSID(a), Self::WindowsSID(b)) => a == b,
            _ => false,
        }
    }
}

impl<'a> Eq for AccessKeyRef<'a> {}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AccessIdent {
    pub name: TwoString,
    pub domain: Capable<DomainsCapable, Option<TwoString>>,
    pub id: Capable<UnixIDsCapable, UID>,
    pub sid: Capable<WindowsSIDsCapable, StringSID>,
}

impl AccessIdent {
    pub fn name(&self) -> TwoStr<'_> {
        self.name.as_two_str()
    }

    pub fn domain(&self) -> Capable<DomainsCapable, Option<TwoStr<'_>>> {
        self.domain.as_deref()
    }

    pub fn unix_id(&self) -> Capable<UnixIDsCapable, UnixID> {
        self.id
    }
    
    pub fn windows_sid(&self) -> Capable<WindowsSIDsCapable, &StrSID> {
        self.sid.as_deref()
    }
}

pub trait HasAccessIdent: AsAID {
    fn ident(&self) -> &AccessIdent;

    fn to_identifiers(&self) -> Vec<AccessKey> {
        let ident = self.ident();
        let mut vec = vec![AccessKey::Name(ident.name().into())];

        if let Capable::Capable(Some(domain)) = ident.domain() {
            vec.push(AccessKey::QualifiedName(
                domain.into(),
                ident.name().into(),
            ));
        }

        if let Capable::Capable(id) = ident.unix_id() {
            vec.push(AccessKey::UnixID(id));
        }

        vec
    }
    
    fn to_id(&self) -> AccessId {
        let ident = self.ident();
        if let Capable::Capable(unix_id) = ident.unix_id() {
            AccessId::UnixID(unix_id)
        } else if let Capable::Capable(sid) = ident.windows_sid() {
            AccessId::WindowsSID(sid.to_string())
        } else {
            unreachable!("Ident does not have an ID key")
        }
    }

    fn to_id_key(&self) -> AccessKey {
        let ident = self.ident();

        if let Capable::Capable(unix_id) = ident.unix_id() {
            AccessKey::UnixID(unix_id)
        } else if let Capable::Capable(sid) = ident.windows_sid() {
            AccessKey::WindowsSID(sid.to_string())
        } else {
            unreachable!("Ident does not have an ID key")
        }
    }
}


#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct User {
    pub ident: AccessIdent,
    pub primary_group_id: Capable<PrimaryUserGroupsCapable, AccessId>,
}

impl User {
    pub fn primary_group_id(&self) -> Capable<PrimaryUserGroupsCapable, AccessIdRef<'_>> {
        let r = self.primary_group_id.as_ref().map(|id| id.as_aid());
        r
    }
}

pub trait UserTrait: HasAccessIdent {
    fn username(&self) -> TwoStr<'_> {
        self.ident().name()
    }

    fn unix_id(&self) -> Capable<UnixIDsCapable, UnixID> {
        self.ident().unix_id()
    }
    
    fn windows_sid(&self) -> Capable<WindowsSIDsCapable, &StrSID> {
        self.ident().windows_sid()
    }
    
    #[cfg(target_family = "unix")]
    fn uid(&self) -> BridgeResult<UID> {
        self.ident().unix_id().ok_into()
    }
    
    #[cfg(target_family = "windows")]
    fn sid(&self) -> BridgeResult<&StrSID> {
        self.ident().windows_sid().ok_into()
    }

    fn domain(&self) -> Capable<DomainsCapable, Option<TwoStr<'_>>> {
        self.ident().domain()
    }
}

impl HasAccessIdent for User {
    fn ident(&self) -> &AccessIdent {
        &self.ident
    }
}

impl UserTrait for User {}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UserGroup {
    pub ident: AccessIdent,
}


pub trait UserGroupTrait: HasAccessIdent {
    fn groupname(&self) -> TwoStr<'_> {
        self.ident().name()
    }

    fn domain(&self) -> Capable<DomainsCapable, Option<TwoStr<'_>>> {
        self.ident().domain()
    }

    fn windows_sid(&self) -> Capable<WindowsSIDsCapable, &StrSID> {
        self.ident().windows_sid()
    }
    
    #[cfg(target_family = "unix")]
    fn gid(&self) -> BridgeResult<GID> {
        self.ident().unix_id().ok_into()
    }
    
    #[cfg(target_family = "windows")]
    fn sid(&self) -> BridgeResult<&StrSID> {
        self.ident().windows_sid().ok_into()
    }
}

impl HasAccessIdent for UserGroup {
    fn ident(&self) -> &AccessIdent {
        &self.ident
    }
}

impl UserGroupTrait for UserGroup {}

pub trait AsAID {
    fn as_aid(&self) -> AccessIdRef<'_>;
}

impl AsAID for AccessId {
    fn as_aid(&self) -> AccessIdRef<'_> {
        match self {
            AccessId::UnixID(id) => AccessIdRef::UnixID(*id),
            AccessId::WindowsSID(sid) => AccessIdRef::WindowsSID(sid.as_str()),
        }
    }
}

impl AsAID for AccessIdent {
    fn as_aid(&self) -> AccessIdRef<'_> {
        if let Capable::Capable(unix_id) = self.unix_id() {
            AccessIdRef::UnixID(unix_id)
        } else if let Capable::Capable(sid) = self.windows_sid() {
            AccessIdRef::WindowsSID(sid)
        } else {
            unreachable!("Ident does not have an ID key")
        }
    }
}

impl<'a> AsAID for AccessIdRef<'a> {
    fn as_aid(&self) -> AccessIdRef<'_> {
        match self {
            AccessIdRef::UnixID(id) => AccessIdRef::UnixID(*id),
            AccessIdRef::WindowsSID(sid) => AccessIdRef::WindowsSID(sid),
        }
    }
}

impl<'a> AsAID for &AccessIdRef<'a> {
    fn as_aid(&self) -> AccessIdRef<'_> {
        match self {
            AccessIdRef::UnixID(id) => AccessIdRef::UnixID(*id),
            AccessIdRef::WindowsSID(sid) => AccessIdRef::WindowsSID(sid),
        }
    }
}

impl<'a> AsAID for Arc<AccessId> {
    fn as_aid(&self) -> AccessIdRef<'_> {
        self.as_ref().as_aid()
    }
}

impl<'a> AsAID for &Arc<AccessId> {
    fn as_aid(&self) -> AccessIdRef<'_> {
        self.as_ref().as_aid()
    }
}

impl AsAID for User {
    fn as_aid(&self) -> AccessIdRef<'_> {
        self.ident.as_aid()
    }
}

impl AsAID for &User {
    fn as_aid(&self) -> AccessIdRef<'_> {
        self.ident.as_aid()
    }
}

impl AsAID for UserGroup {
    fn as_aid(&self) -> AccessIdRef<'_> {
        self.ident.as_aid()
    }
}

impl AsAID for &UserGroup {
    fn as_aid(&self) -> AccessIdRef<'_> {
        self.ident.as_aid()
    }
}

impl<'a> AsAccess<'a> for AccessIdRef<'a> {
    fn as_key_ref(&'a self) -> AccessKeyRef<'a> {
        match self {
            AccessIdRef::UnixID(uid) => AccessKeyRef::UnixID(*uid),
            AccessIdRef::WindowsSID(s) => AccessKeyRef::WindowsSID(*s),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BasicPermission {
    Read,
    Write,
    Execute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BasicPermissionWho {
    User,
    Group,
    Public,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BasicPermissionSet {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

impl BasicPermissionSet {
    pub const DEFAULT: Self = Self {
        read: false,
        write: false,
        execute: false,
    };
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BasicPermissionMode {
    pub user: BasicPermissionSet,
    pub group: BasicPermissionSet,
    pub public: BasicPermissionSet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BasicPermissionGrant {
    Give(BasicPermission),
    Take(BasicPermission),
}

impl BasicPermissionMode {
    pub const DEFAULT: Self = Self {
        user: BasicPermissionSet::DEFAULT,
        group: BasicPermissionSet::DEFAULT,
        public: BasicPermissionSet::DEFAULT,
    };
    
    pub fn to_unix_file_mode(&self) -> UnixFileMode {
        let mut mode: UnixFileMode = 0;
        if self.user.read {
            mode |= 0o400;
        }
        if self.user.write {
            mode |= 0o200;
        }
        if self.user.execute {
            mode |= 0o100;
        }
        if self.group.read {
            mode |= 0o040;
        }
        if self.group.write {
            mode |= 0o020;
        }
        if self.group.execute {
            mode |= 0o010;
        }
        if self.public.read {
            mode |= 0o004;
        }
        if self.public.write {
            mode |= 0o002;
        }
        if self.public.execute {
            mode |= 0o001;
        }
        
        mode
    }
}

impl From<BasicPermissionSet> for Vec<BasicPermissionGrant> {
    fn from(value: BasicPermissionSet) -> Self {
        let mut vec = Vec::new();
        if value.read {
            vec.push(BasicPermissionGrant::Give(BasicPermission::Read));
        }
        if value.write {
            vec.push(BasicPermissionGrant::Give(BasicPermission::Write));
        }
        if value.execute {
            vec.push(BasicPermissionGrant::Give(BasicPermission::Execute));
        }
        vec
    }
}

impl AsRef<BasicPermissionMode> for BasicPermissionMode {
    fn as_ref(&self) -> &Self {
        self
    }
}