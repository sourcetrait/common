use crate::*;

/// Standalone top-level testing group.
///
/// Lives outside of the Module -> Test heirarchy.
///
/// Useful for grouping common fixtures, setup/teardown, and temp file operations.
#[derive(Debug)]
pub struct TestGroup {
    pub(crate) use_case: UseCase,
    pub(crate) namepath: Namepath,
    pub(crate) temp_dir: Option<PathBuf>,
    pub(crate) base_temp_dir: Option<PathBuf>,
    pub(crate) fixture_dir: Option<PathBuf>,
}

impl TestGroup {
    #[inline]
    pub fn as_testable(&self) -> Testable<'_, '_, '_> {
        Testable::Group(self)
    }
    
    pub fn base_temp_dir(&self) -> &Path {
        &self.base_temp_dir.as_ref().context("Module `base temp dir` is not configured").unwrap()
    }
}

impl Testing for TestGroup {
    fn fixture_dir(&self) -> &Path {
        &self.fixture_dir.as_ref().context("Group `fixture dir` is not configured").unwrap()
    }
    
    #[inline]
    fn kind(&self) -> TestingKind {
        TestingKind::Group
    }
    
    fn namepath(&self) -> &Namepath {
        &self.namepath
    }

    fn temp_dir(&self) -> &Path {
        self.temp_dir.as_ref().context("Group `temp dir` is not configured").unwrap()
    }
    
    fn use_case(&self) -> UseCase {
         self.use_case
    }
}

/// Constructs a [TestGroup]
pub struct GroupBuilder<'func> {
    pub(crate) package_name: &'static str,
    pub(crate) use_case: UseCase,
    pub(crate) group_path: &'static str,
    pub(crate) base_temp_dir: PathBuf,
    pub(crate) using_temp_dir: bool,
    pub(crate) using_fixture_dir: bool,
    pub(crate) skip_temp_dir_teardown: bool,
    pub(crate) setup_func: Option<Box<dyn FnOnce(&mut TestGroup) + 'func>>,
    pub(crate) static_teardown_func: Option<extern "C" fn()>,
}

impl<'func> GroupBuilder<'func> {
    pub fn new(package_name: &'static str, use_case: UseCase, group_path: &'static str) -> Self {
        Self {
            package_name,
            use_case,
            group_path,
            base_temp_dir: std::env::temp_dir(),
            using_temp_dir: false,
            using_fixture_dir: false,
            skip_temp_dir_teardown: false,
            setup_func: None,
            static_teardown_func: None,
        }
    }

    pub fn build(mut self) -> TestGroup {
        let namepath = Namepath::new_group(self.package_name, self.use_case, self.group_path)
            .expect("Invalid namepath");

        let base_temp_dir;
        let temp_dir = if self.using_temp_dir {
            let dirname = namepath.full_path_to_squashed_slug();
            base_temp_dir = Some(create_random_subdir(&self.base_temp_dir, &dirname) // todo: use squashed prefix
                .context(format!("Unable to create temporary directory in base: {}", &self.base_temp_dir.to_str().unwrap()))
                .unwrap() );

            Some(build_temp_dir(&namepath, &base_temp_dir.as_ref().unwrap()) )
        } else {
            base_temp_dir = None;
            None
        };

        let fixture_dir = if self.using_fixture_dir {
            Some(build_fixture_dir(&namepath))
        } else {
            None
        };

        let mut group = TestGroup {
            namepath,
            use_case: self.use_case,
            base_temp_dir,
            temp_dir,
            fixture_dir,
        };

        if let Some(setup_func) = self.setup_func {
            setup_func(&mut group);
        }
        
        let teardown_temp_dir = if self.skip_temp_dir_teardown {
            if let Some(tmpdir) = &group.base_temp_dir {
                // Log that we're skipping teardown of the temp dir
                println!("TESTING: {} :: Skipped teardown of temp_dir:\n  {}",
                    group.namepath,
                    tmpdir.display());
            }

            None
        } else {
            group.base_temp_dir.clone()
        };

        let teardown = Teardown {
            base_temp_dir: teardown_temp_dir,
            func: self.static_teardown_func.take()
        };

        teardown_queue_push(teardown);

        group
    }

    pub fn base_temp_dir<P>(mut self, dir: &P) -> Self
    where
        P: ?Sized + AsRef<OsStr>
    {
        let dir = PathBuf::from(dir);
        let dir = dunce::canonicalize(&dir)
            .context(format!("Base temporary directory does not exist: {}", &dir.to_str().unwrap()))
            .unwrap();

        self.base_temp_dir = dir;
        self
    }
    
    pub fn skip_temp_dir_teardown(mut self, skip: bool) -> Self {
        self.skip_temp_dir_teardown = skip;
        self
    }

    pub fn using_temp_dir(mut self) -> Self {
        self.using_temp_dir = true;
        self
    }

    pub fn using_fixture_dir(mut self) -> Self {
        self.using_fixture_dir = true;
        self
    }

    pub fn setup(mut self, func: impl FnOnce(&mut TestGroup) + 'func) -> Self {
        self.setup_func = Some(Box::new(func));
        self
    }

    pub fn teardown_static(mut self, func: extern "C" fn()) -> Self {
        self.static_teardown_func = Some(func);
        self
    }
}

/// Lazy-locked wrapper for [TestGroup]
///
/// Typically constructed using the [group!()] macro.
///
/// Statically associated with a Rust module.
pub struct Group(LazyLock<TestGroup>);

impl Deref for Group {
    type Target = LazyLock<TestGroup>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Group {
    pub const fn new(func: fn() -> TestGroup) -> Self {
        Self(LazyLock::new(func))
    }
}

/// Constructs a [TestGroup] and wraps it in [Group]
/// 
/// ## Forms
/// - Basic:
///   - `group!(namepath: &str, use_case_variant: testing::UseCase::{Variant})`
/// - Builder:
///   - `group!(namepath: &str, use_case_variant: testing::UseCase::{Variant}, { builder method calls ... })`
/// 
/// ## Examples
/// ### Basic 
/// ```rust,ignore
/// static MYGROUP: testing::Group = testing::group!("my/group", Integration);
/// ```
/// ### Builder
/// ```rust,ignore
/// static MYGROUP: testing::Group = testing::group!("my/group", Unit, {
///     .using_fixture_dir()
///     .setup(|_this| {
///         dbg!("hello");
///     })
/// });
/// ```
#[macro_export]
macro_rules! group {
    ($n:expr, $u:tt, {$($b:tt)+}) => {
        $crate::Group::new(|| {
            $crate::GroupBuilder::new(env!("CARGO_PKG_NAME"), $crate::UseCase::$u, $n)
            $($b)+
                .build()
        })
    };
    ($n:expr, $u:tt) => {
        $crate::Group::new(|| {
            $crate::GroupBuilder::new(env!("CARGO_PKG_NAME"), $crate::UseCase::$u, $n).build()
        })
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    static _GROUP_NEW: Group = Group::new(|| {
        GroupBuilder::new(env!("CARGO_PKG_NAME"), UseCase::Unit, "group/builder")
            .setup(|_| {
                println!("setup called");
            })
            .build()
    });

    static _GROUP_MACRO: Group = group!("group/macro", Unit, {
        .using_temp_dir()
        .setup(|_| {})
    });

    static GROUP_BASIC: Group = group!("group/basic", Unit);

    static GROUP_WITH_DIRS: Group = group!("group/with_dirs", Unit, {
        .using_fixture_dir()
        .using_temp_dir()
    });

    // Group not configured with a temp dir should panic when attempting to access it
    #[test] #[should_panic]
    fn test_temp_dir_unconfigured_access() {
        GROUP_BASIC.temp_dir();  // should panic
    }

    #[test]
    fn test_temp_dir_using() {
        assert!(GROUP_WITH_DIRS.temp_dir().exists(),
            "Group configured with `using_temp_dir()` should create the directory on construction if it does not exist.");
    }

    // Group not configured with a fixture dir should panic when attempting to access it
    #[test] #[should_panic]
    fn test_fixture_dir_unconfigured_access() {
        GROUP_BASIC.fixture_dir(); // should panic
    }

    // Fixture path should exist for Group configured with `using_fixture_dir()`
     #[test]
    fn test_fixture_dir_using() {
        assert!(GROUP_WITH_DIRS.fixture_dir().exists(),
            "Fixture path should exist for Group configured with `using_fixture_dir()`");
    }

    // SAFETY: This can only be called once, by `test_setup_function()`. Not thread safe.
    static mut SETUP_FUNC_CALLED: bool = false;
    fn setup_func(_group: &mut TestGroup) {
        unsafe {
            SETUP_FUNC_CALLED = true;
        }
    }

    static GROUP_WITH_SETUP: Group = group!("group/with_setup", Unit, {
        .setup(setup_func)
    });

    // Group setup function should be ran on construction.
    #[test]
    fn test_setup_function() {
        let _ = GROUP_WITH_SETUP.use_case(); // lazy initialize

        unsafe {
            assert!(SETUP_FUNC_CALLED,
                "Group setup function should be ran on construction.");
        }
    }

    // Group setup closure should be ran on construction.
    #[test]
    fn test_setup_closure() {
        let mut setup_closure_called = false;
        let _group: TestGroup = GroupBuilder::new(env!("CARGO_PKG_NAME"), UseCase::Unit, "group/with_closure")
            .setup(|_| {
                setup_closure_called = true;
            })
            .build();

        assert!(setup_closure_called,
            "Group setup closure should be ran on construction.");
    }

    // only way to test this is using `cargo test -- --show-output`
    extern "C" fn teardown_func() {
        println!("STATIC_GROUP: teardown_static() ran");
    }

    static GROUP_WITH_TEARDOWN: Group = group!("group/with_teardown", Unit, {
        .teardown_static(teardown_func)
    });

    // Group teardown function should be ran on destruction.
    #[test]
    fn test_teardown_function() {
        let _ = GROUP_WITH_TEARDOWN.use_case(); // force lazy init
    }
}
