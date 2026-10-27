use crate::*;

const DOT_ONCE: &'static str = ".once";
const CACHE: &'static str = "cache";
const DOT_GIT: &'static str = ".git";
const CONFIG: &'static str = "config";
const DATA: &'static str = "data";
const SECRETS: &'static str = "secrets";
const RUNTIME: &'static str = "runtime";
const STATE: &'static str = "state";

/// Represents paths for a specific application, including any application-
/// specific path namespacing.
/// 
/// For implementation, it's recommended that well-known filepaths and
/// sub-directories have their own methods.
/// 
/// # Example: Namespacing
/// Application uses a path suffix of "myorg/mysuite/myapp" for everything.  
/// [Self::config_dir] on Linux: $HOME/.config/myorg/mysuite/myapp 
/// 
/// # Example: Test fixtures
/// An integration test passes a single directory to be used for everything.  
/// [Self::config_dir], [Self::data_dir], etc on Linux: /tmp/mytest
pub trait AppPaths {
    fn cache_dir(&self) -> PathBuf;
    fn config_dir(&self) -> PathBuf;
    fn config_secrets_dir(&self) -> PathBuf;
    fn data_dir(&self) -> PathBuf;
    fn state_dir(&self) -> PathBuf;
    fn state_secrets_dir(&self) -> PathBuf;
    fn runtime_dir(&self) -> PathBuf;
}

pub trait HasAppPaths {
    fn app_paths(&self) -> &impl AppPaths;
    fn cache_dir(&self) -> PathBuf { self.app_paths().cache_dir() }
    fn config_dir(&self) -> PathBuf { self.app_paths().config_dir() }
    fn config_secrets_dir(&self) -> PathBuf { self.app_paths().config_secrets_dir() }
    fn data_dir(&self) -> PathBuf { self.app_paths().data_dir() }
    fn state_dir(&self) -> PathBuf { self.app_paths().state_dir() }
    fn state_secrets_dir(&self) -> PathBuf { self.app_paths().state_secrets_dir() }
    fn runtime_dir(&self) -> PathBuf { self.app_paths().runtime_dir() }
}

#[derive(Clone)]
pub struct BasePaths(pub(crate) Either<Arc<directories::BaseDirs>, PathBuf>);

impl BasePaths {
    pub fn sys() -> Self {
        PLATFORM.path().base_paths().expect("Unsupported OS")
    }
    
    pub fn once(dir: PathBuf) -> Self {
        Self(Either::B(dir))
    }
}

impl Default for BasePaths {
    fn default() -> Self {
        if !cfg!(debug_assertions) {
            return Self::sys(); 
        }
        
        let Ok(cwd) = env::current_dir() else {
            return Self::sys(); 
        };
        
        let Ok(Some(exe_dir)) = env::current_exe()
            .map(|f| f.parent().map(|f| f.to_path_buf()))
        else {
            return Self::sys();
        };
                
        let Some(git_root) = stdx::fs::find_parent_dir(&exe_dir, DOT_GIT) else {
            return Self::sys();
        };
        
        if cwd.strip_prefix(&git_root).is_err() {
            return Self::sys();
        }
        
        match git_root.join(DOT_ONCE).canonicalize() {
            Ok(dir) => Self::once(dir),
            Err(_) => Self::sys(),
        }
    }
}

impl AppPaths for BasePaths {
    fn cache_dir(&self) -> PathBuf {
        match &self.0 {
            Either::A(base) => base.cache_dir().into(),
            Either::B(path) => path.join(CACHE),
        }
    }
    
    fn config_dir(&self) -> PathBuf {
        match &self.0 {
            Either::A(base) => base.config_dir().into(),
            Either::B(path) => path.join(CONFIG),
        }
    }

    fn config_secrets_dir(&self) -> PathBuf {
        match &self.0 {
            Either::A(base) => base.config_dir().join(SECRETS),
            Either::B(path) => path.join(CONFIG).join(SECRETS),
        }
    }
    
    fn data_dir(&self) -> PathBuf {
        match &self.0 {
            Either::A(base) => base.data_dir().into(),
            Either::B(path) => path.join(DATA),
        }
    }
    
    fn state_dir(&self) -> PathBuf {
        match &self.0 {
            Either::A(base) => base.state_dir()
                .unwrap_or_else(|| base.cache_dir())
                .into(),
            Either::B(path) => path.join(STATE),
        }
    }
    
    fn state_secrets_dir(&self) -> PathBuf {
        match &self.0 {
            Either::A(base) => base.state_dir()
                .unwrap_or_else(|| base.cache_dir())
                .join(SECRETS)
                .into(),
            Either::B(path) => path.join(STATE).join(SECRETS),
        }
    }

    fn runtime_dir(&self) -> PathBuf {
        match &self.0 {
            Either::A(base) => base.runtime_dir()
                .unwrap_or_else(|| base.cache_dir())
                .into(),
            Either::B(path) => path.join(RUNTIME),
        }
    }
}

pub enum DefaultAppPaths {
    Default(&'static str),
    Determined(BasePaths, &'static Path),
    Dir(PathBuf),
}

impl DefaultAppPaths {
    pub fn determine(self) -> AppPathRouter {
        match self {
            Self::Default(suffix) => AppPathRouter::Determined(BasePaths::default(), Path::new(suffix)),
            Self::Determined(b,d) => AppPathRouter::Determined(b,d),
            Self::Dir(d) => AppPathRouter::Dir(d),
        }
    }
}

impl From<DefaultAppPaths> for AppPathRouter {
    fn from(value: DefaultAppPaths) -> Self {
        value.determine()
    }
}

#[derive(Clone)]
pub enum AppPathRouter {
    Determined(BasePaths, &'static Path),
    Dir(PathBuf),
}

impl AppPaths for AppPathRouter {
    fn cache_dir(&self) -> PathBuf {
        match self {
            Self::Determined(b, p) => b.cache_dir().join(p), 
            Self::Dir(p) => p.clone(),
        }
    }

    fn config_dir(&self) -> PathBuf {
        match self {
            Self::Determined(b, p) => b.config_dir().join(p), 
            Self::Dir(p) => p.clone(),
        }
    }
    
    fn config_secrets_dir(&self) -> PathBuf {
        match self {
            Self::Determined(b, p) => b.config_secrets_dir().join(p), 
            Self::Dir(p) => p.clone(),
        }
    }

    fn data_dir(&self) -> PathBuf {
        match self {
            Self::Determined(b, p) => b.data_dir().join(p), 
            Self::Dir(p) => p.clone(),
        }
    }

    fn state_dir(&self) -> PathBuf {
        match self {
            Self::Determined(b, p) => b.state_dir().join(p), 
            Self::Dir(p) => p.clone(),
        }
    }
    
    fn state_secrets_dir(&self) -> PathBuf {
        match self {
            Self::Determined(b, p) => b.state_secrets_dir().join(p), 
            Self::Dir(p) => p.clone(),
        }
    }

    fn runtime_dir(&self) -> PathBuf {
        match self {
            Self::Determined(b, p) => b.runtime_dir().join(p), 
            Self::Dir(p) => p.clone(),
        }
    }
}

pub enum WhichAppFiles<P: HasAppPaths> {
    Determined(BasePaths, &'static Path),
    Dir(PathBuf),
    Paths(P)
}
