use crate::*;

/// Represents a Rust module that contains tests.
///
/// Provides parent configuration for each [Test]:
/// - Base [namepaths](Namepath)
/// - Base fixture and tmp directories
/// - The intended [UseCase] for each test
///
/// Each [Test] has one parent [TestModule]
///
/// This is statically associated with a module using a [Module] wrapper.
#[derive(PartialEq, Eq, Debug)]
pub struct TestModule {
    pub(crate) namepath: Namepath,
    pub(crate) use_case: UseCase,
    pub(crate) base_temp_dir: Option<PathBuf>,
    pub(crate) temp_dir: Option<PathBuf>,
    pub(crate) fixture_dir: Option<PathBuf>,
}

impl TestModule {
    #[inline]
    pub fn as_testable(&self) -> Testable<'_, '_, '_> {
        Testable::Module(self)
    }

    pub fn base_temp_dir(&self) -> &Path {
        &self.base_temp_dir.as_ref().context("Module `base temp dir` is not configured").unwrap()
    }

    /// Creates a [TestBuilder].
    pub fn test_builder(&self, name: &'static str) -> TestBuilder<'_,'_> {
        TestBuilder::new(&self, name)
    }
}

impl Testing for TestModule {
    fn fixture_dir(&self) -> &Path {
        self.fixture_dir.as_ref().context("Module `fixture dir` is not configured").unwrap()
    }
    
    #[inline]
    fn kind(&self) -> TestingKind {
        TestingKind::Group
    }
    
    fn namepath(&self) -> &Namepath {
        &self.namepath
    }

    fn temp_dir(&self) -> &Path {
        self.temp_dir.as_ref().context("Module `temp dir` is not configured").unwrap()
    }
    
    fn use_case(&self) -> UseCase {
        self.use_case
    }
}

/// Builds a new [TestModule]
///
/// The helper macro [module!()] is typically preferred over using this
/// directly.
pub struct ModuleBuilder<'func> {
    pub(crate) use_case: UseCase,
    pub(crate) package_name: &'static str,
    pub(crate) module_path: &'static str,
    pub(crate) base_temp_dir: PathBuf,
    pub(crate) using_temp_dir: bool,
    pub(crate) skip_temp_dir_teardown: bool,
    pub(crate) using_fixture_dir: bool,
    pub(crate) setup_func: Option<Box<dyn FnOnce(&mut TestModule) + 'func>>,
    pub(crate) static_teardown_func: Option<extern "C" fn()>,
}

impl<'func> ModuleBuilder<'func> {
    pub fn new(package_name: &'static str, use_case: UseCase, module_path: &'static str) -> Self {
        ModuleBuilder {
            package_name,
            use_case,
            module_path,
            base_temp_dir: std::env::temp_dir(),
            using_temp_dir: false,
            skip_temp_dir_teardown: false,
            using_fixture_dir: false,
            setup_func: None,
            static_teardown_func: None,
        }
    }

    /// Builds the [TestModule]
    ///
    /// Creates the temp directory, if requested.
    /// Runs the setup function, if provided.
    /// Registers a shutdown hook to handle internal cleanup (temp directories)
    /// Register a shutdown hook for the custom teardown function, if provided
    pub fn build(mut self) -> TestModule {
        let namepath = Namepath::new_module(self.package_name, self.use_case, self.module_path)
            .expect("Invalid namepath for testing module");

        let base_temp_dir;
        let temp_dir = if self.using_temp_dir {
            let dirname = namepath.full_path_to_squashed_slug();
            base_temp_dir = Some(create_random_subdir(&self.base_temp_dir, &dirname) // todo: use squashed prefix
                .context(format!("Unable to create temporary directory in base: {}", &self.base_temp_dir.to_str().unwrap()))
                .unwrap() );

            let tmpdir = build_temp_dir(&namepath, &base_temp_dir.as_ref().unwrap());
            Some(tmpdir)
        } else {
            base_temp_dir = None;
            None
        };

        let fixture_dir = if self.using_fixture_dir {
            Some(build_fixture_dir(&namepath) )
        } else {
            None
        };

        let mut module = TestModule {
            namepath,
            use_case: self.use_case,
            base_temp_dir,
            temp_dir,
            fixture_dir,
        };

        if let Some(setup_fn) = self.setup_func {
            setup_fn(&mut module);
        }

        let teardown_temp_dir = if self.skip_temp_dir_teardown {
            if let Some(tmpdir) = &module.base_temp_dir {
                // Log that we're skipping teardown of the temp dir
                println!("TESTING: {} :: Skipped teardown of temp_dir:\n  {}",
                    module.namepath,
                    tmpdir.display());
            }

            None
        } else {
            module.base_temp_dir.clone()
        };

        let teardown = Teardown {
            base_temp_dir: teardown_temp_dir,
            func: self.static_teardown_func.take()
        };

        teardown_queue_push(teardown);

        module
    }

    pub fn using_fixture_dir(mut self) -> Self {
        self.using_fixture_dir = true;
        self
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

    pub fn using_temp_dir(mut self) -> Self {
        self.using_temp_dir = true;
        self
    }

    pub fn skip_temp_dir_teardown(mut self, skip: bool) -> Self {
        self.skip_temp_dir_teardown = skip;
        self
    }

    pub fn setup(mut self, func: impl FnOnce(&mut TestModule) + 'func) -> Self {
        self.setup_func = Some(Box::new(func));
        self
    }

    pub fn teardown_static(mut self, func: extern "C" fn()) -> Self {
        self.static_teardown_func = Some(func);
        self
    }
}

/// Lazy-locked wrapper for [TestModule].
///
/// Typically, it's constructed using the [module!()] macro. It can also be
/// manually created by passing the result of [ModuleBuilder] to it.
pub struct Module(LazyLock<TestModule>);

impl Deref for Module {
    type Target = LazyLock<TestModule>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Module {
    /// Creates a lazy-locked wrapper over [TestModule]
    pub const fn new(func: fn() -> TestModule) -> Self {
        Self(LazyLock::new(func))
    }
}

pub struct TestModuleWith<H> {
    inner: TestModule,
    harness: H,
}

impl<H> TestModuleWith<H> {
    pub fn harness(&self) -> &H {
        &self.harness
    }
}

impl<H> Deref for TestModuleWith<H> {
    type Target = TestModule;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub struct ModuleBuilderWith<'func, H> {
    pub(crate) inner: ModuleBuilder<'func>,
    pub(crate) harness_setup_fn: Option<Box<dyn FnOnce(&mut TestModule) -> H + 'func>>,
}

impl<'func, H> ModuleBuilderWith<'func, H> {
    pub fn new(package_name: &'static str, use_case: UseCase, module_path: &'static str) -> Self {
        let inner = ModuleBuilder::new(package_name, use_case, module_path);
        Self {
            inner,
            harness_setup_fn: None,
        }
    }
    
    pub fn setup(mut self, func: impl FnOnce(&mut TestModule) -> H + 'func) -> Self {
        self.harness_setup_fn = Some(Box::new(func));
        self
    }
    
    pub fn base_temp_dir<P>(mut self, dir: &P) -> Self
    where
        P: ?Sized + AsRef<OsStr>
    {
        self.inner = self.inner.base_temp_dir(dir);
        self
    }
    
    pub fn using_fixture_dir(mut self) -> Self {
        self.inner = self.inner.using_fixture_dir();
        self
    }
    pub fn using_temp_dir(mut self) -> Self {
        self.inner = self.inner.using_temp_dir();
        self
    }

    pub fn skip_temp_dir_teardown(mut self, skip: bool) -> Self {
        self.inner = self.inner.skip_temp_dir_teardown(skip);
        self
    }

    pub fn teardown_static(mut self, func: extern "C" fn()) -> Self {
        self.inner = self.inner.teardown_static(func);
        self
    }

    pub fn build(self) -> TestModuleWith<H> {
        let mut inner = self.inner.build();
        
        let harness_fn = self.harness_setup_fn
            .expect(".setup_harness() is required");
        
        let harness = (harness_fn)(&mut inner);

        TestModuleWith {
            inner,
            harness,
        }
    }
} 

/// Lazy-locked wrapper for [TestHarnessModule].
///
/// Typically, it's constructed using the [module!()] macro. It can also be
/// manually created by passing the result of [ModuleBuilder] to it.
pub struct ModuleWith<H>(LazyLock<TestModuleWith<H>>);

impl<H> Deref for ModuleWith<H> {
    type Target = LazyLock<TestModuleWith<H>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<H> ModuleWith<H> {
    /// Creates a lazy-locked wrapper over [TestModule]
    pub const fn new(func: fn() -> TestModuleWith<H>) -> Self {
        Self(LazyLock::new(func))
    }
}

/// Constructs a [TestModule] and wraps it inside a lazy-locked [Module]
/// 
/// The name of the static variable should be `TESTING` for standard interop.
///
/// ## Forms
/// - Basic:
///   - `module!(use_case_variant: testing::UseCase::{Variant})`
/// - Builder:
///   - `module!(use_case_variant: testing::UseCase::{Variant}, { builder method calls ... })`
/// 
/// ## Examples
/// ### Basic 
/// ```rust,ignore
/// static TESTING: testing::Module = testing::module!(Integration);
/// ```
/// ### Builder
/// ```rust,ignore
/// static TESTING: testing::Group = testing::module!(Unit, {
///     .using_fixture_dir()
///     .setup(|_this| {
///         dbg!("hello");
///     })
/// });
/// ```
#[macro_export]
macro_rules! module {
    ($u:tt, {$($b:tt)+}) => {
        $crate::Module::new(|| {
            $crate::ModuleBuilder::new(env!("CARGO_PKG_NAME"), $crate::UseCase::$u, module_path!())
            $($b)+
                .build()
        })
    };
    ($u:tt) => {
        $crate::Module::new(|| {
            $crate::ModuleBuilder::new(env!("CARGO_PKG_NAME"), $crate::UseCase::$u, module_path!()).build()
        })
    };
}

#[macro_export]
macro_rules! module_with {
    ($u:tt, {$($b:tt)+}) => {
        $crate::ModuleWith::new(|| {
            $crate::ModuleBuilderWith::new(env!("CARGO_PKG_NAME"), $crate::UseCase::$u, module_path!())
            $($b)+
                .build()
        })
    };
    ($u:tt) => {
        $crate::ModuleWith::new(|| {
            $crate::ModuleBuilderWith::new(env!("CARGO_PKG_NAME"), $crate::UseCase::$u, module_path!()).build()
        })
    };
}


#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::LazyLock;
    use crate::*;

    #[test] #[should_panic]
    // Should panic if attempting to retrieve the temp_dir() without having configured one manually or by calling ensure_temp_dir().
    fn test_temp_dir_unconfigured() {
        let module = module!(Unit);
        module.temp_dir();  // should panic
    }

    // Should panic if attempting to retrieve the fixture_dir() without having configured one manually or by calling ensure_fixture_dir().
    #[test] #[should_panic]
    fn test_fixture_dir_unconfigured() {
        let module = module!(Unit);
        module.fixture_dir(); // should panic
    }

    // Module base temp dir should be inaccessible if not using a temp dir.
    #[test] #[should_panic]
    fn test_base_temp_dir_unconfigured_temp_dir() {
        module!(Unit, {
            .base_temp_dir(&std::env::temp_dir())
        }).base_temp_dir();  // should panic
    }

    // Module base temp dir should accept paths of types `Path` and `String`.
    #[test]
    fn test_base_temp_dir() {
        static EXPECTED_BASE_TEMP_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
            let base_temp_dir = std::env::temp_dir()
                .join("sourcetrait_testing_unit_module");

            if !base_temp_dir.exists() {
                std::fs::create_dir(&base_temp_dir).unwrap(); // needs manual teardown
            }

            base_temp_dir.canonicalize().unwrap() // for posterity
        });

        let module = module!(Unit, {
            .base_temp_dir(EXPECTED_BASE_TEMP_DIR.as_path())
            .using_temp_dir()
        });

        assert_eq!(EXPECTED_BASE_TEMP_DIR.as_path(), module.base_temp_dir().parent().unwrap(),
            "Module base temp dir should accept paths of type `Path`." );

        let module = module!(Unit, {
            .base_temp_dir(EXPECTED_BASE_TEMP_DIR.to_str().unwrap())
            .using_temp_dir()
        });

        assert_eq!(EXPECTED_BASE_TEMP_DIR.as_path(), module.base_temp_dir().parent().unwrap(),
            "Module base temp dir should accept paths of type `String`." );


        std::fs::remove_dir_all(EXPECTED_BASE_TEMP_DIR.as_path()).unwrap(); // testing cleanup
    }

    // Module should not allow configuration of base temp dir with a relative path.
    // Only canonical paths are allowed.
    #[test] #[should_panic]
    fn test_base_temp_dir_relative() {
        let module = module!(Unit, {
            .base_temp_dir("tmp")
        });

        let _ = module.namepath(); // force lazy init
    }

    // Module should not allow configuration of a base temp dir with a non-existing path.
    #[test] #[should_panic]
    fn test_base_temp_dir_nonexistant() {
        let module = module!(Unit, {
            .base_temp_dir(&std::env::temp_dir().join("srctraittestingnoandthen"))
        });

        let _ = module.namepath(); // force lazy init
    }

    // Module use-case should match the fascade helper function that was used to create it.
    #[test]
    fn test_use_case() {
        let unit = module!(Unit);
        let integration = module!(Integration);

        assert_eq!(UseCase::Unit, unit.use_case(),
            "Module use-case should match the fascade helper function (Unit) that was used to create it.");
        assert_eq!(UseCase::Integration, integration.use_case(),
            "Module use-case should match the fascade helper function (Integration) that was used to create it.");
    }

    // Module configured with `using_temp_dir()` should have a temp path:
    //     `Module.base_temp_dir() + `Module.namepath().path()`
    // Module configured with `using_temp_dir()` should create the temp directory on construction.
    #[test]
    fn test_temp_dir_using() {
        const MODULE_PATH: &'static str = "sourcetrait_testing::module::test_temp_dir_using";
        const EXPECTED_DIRNAME: &'static str = "unit/module/test_temp_dir_using";
        let unit = ModuleBuilder::new(env!("CARGO_PKG_NAME"), UseCase::Unit, MODULE_PATH)
            .using_temp_dir().build();
        let expected_tmp_dir = PathBuf::from(&unit.base_temp_dir()).join(EXPECTED_DIRNAME);

        assert_eq!(expected_tmp_dir, unit.temp_dir(),
            "Module configured with `using_temp_dir()` should have a temp path: `Module.base_temp_dir() + `Module.namepath().path()`");
        assert!(unit.temp_dir().exists(),
            "Module configured with `using_temp_dir()` should create the temp directory on construction.");
    }

    fn expected_unit_module_fixture_dir() -> PathBuf {
        PathBuf::from(strings::TESTING).join(strings::FIXTURES)
            .join(UseCase::Unit.to_string())
            .join("module")
            .canonicalize()
            .unwrap()
    }

    // Module configured with `using_fixture_dir()` should have a fixture path:
    //     testing / fixtures / `Module.use_case()` / `Module::namepath().dir()`
    // Module configured with `using_fixture_dir()` should have a pre-existing fixture dir
    #[test]
    fn test_fixture_dir_using() {
        let unit = module!(Unit, {
            .using_fixture_dir()
        });

        assert_eq!(expected_unit_module_fixture_dir(), unit.fixture_dir(),
            "Module configured with `using_fixture_dir` should have a fixture path: testing / fixtures / `Module.use_case()` / `Module.namepath().dir()`");
         assert!(unit.fixture_dir().exists(),
            "Module configured with `using_fixture_dir` should have a pre-existing fixture dir");
    }

    static mut SETUP_FUNC_CALLED: bool = false;
    fn setup_func(_module: &mut TestModule) {
        unsafe {
            SETUP_FUNC_CALLED = true;
        }
    }

    #[test]
    // Should run a setup function
    fn test_setup_function() {
        let module = module!(Unit, {
            .setup(setup_func)
        });

        let _ = module.namepath(); // lazy init

        unsafe {
            assert!(SETUP_FUNC_CALLED);
        }
    }

    #[test]
    // Should run a setup closure
    fn test_setup_closure() {
        let mut setup_closure_called = false;

        let _module = ModuleBuilder::new(env!("CARGO_PKG_NAME"), UseCase::Unit, module_path!())
            .setup(|_| {
                setup_closure_called = true;
            })
            .build();

        assert!(setup_closure_called);
    }

    extern "C" fn static_teardown_func() {
        println!("STATIC_MODULE: teardown_static() ran");
    }

    #[test]
    // Should set a teardown hook. Not testing the actual atexit call here.
    fn test_teardown_static() {
        let _module = module!(Unit, {
            .teardown_static(static_teardown_func)
        });
    }
}
