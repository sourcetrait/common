use crate::*;

/// Configuraiton for a single unit or integration test.
///
/// It has a parent [TestModule] from which it may inherit configuration.
pub struct Test<'module,'func> {
    pub(crate) module: &'module TestModule,
    pub(crate) namepath: Namepath,
    pub(crate) temp_dir: Option<PathBuf>,
    pub(crate) fixture_dir: Option<PathBuf>,
    pub(crate) teardown_func: Option<Box<dyn FnOnce(&mut Test) + 'func>>,
}

impl std::fmt::Debug for Test<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let teardown_fn_dbg = if self.teardown_func.is_some() {
            "Some(fn(...))"
        } else {
            "None"
        };
        
        f.debug_struct("Test")
            .field("namepath", &self.namepath)
            .field("module", &self.module)
            .field("temp_dir", &self.temp_dir)
            .field("fixture_dir", &self.fixture_dir)
            .field("teardown_func", &teardown_fn_dbg)
            .finish()
    }
}

impl<'module,'func> Test<'module,'func> {
    #[inline]
    pub fn as_testable(&self) -> Testable<'_, 'module, 'func> {
        Testable::Test(self)
    }
    
    /// The parent [TestModule] of this test.
    pub fn module(&self) -> &'module TestModule {
        &self.module
    }

    fn teardown(&mut self) {
        if let Some(teardown_fn) = self.teardown_func.take() {
            teardown_fn(self);
        }
    }
}

impl<'module,'func> Testing for Test<'module,'func> {
    fn fixture_dir(&self) -> &Path {
        &self.fixture_dir.as_ref()
            .context("Test `fixture dir` is not configured").unwrap()
    }
    
    #[inline]
    fn kind(&self) -> TestingKind {
        TestingKind::Test
    }
    
    fn namepath(&self) -> &Namepath {
        &self.namepath
    }

    fn temp_dir(&self) -> &Path {
        self.temp_dir.as_ref()
            .context("Test `temp dir` is not configured").unwrap()
    }
    
    fn use_case(&self)-> UseCase {
        self.module.use_case
    }
}

impl<'module,'func> Drop for Test<'module,'func> {
    fn drop(&mut self) {
        self.teardown();
    }
}

/// Constructs a [Test]
pub struct TestBuilder<'module,'func> {
    pub(crate) name: &'static str,
    pub(crate) module: &'module TestModule,
    pub(crate) using_temp_dir: bool,
    pub(crate) inherit_temp_dir: bool,
    pub(crate) using_fixture_dir: bool,
    pub(crate) inherit_fixture_dir: bool,
    pub(crate) setup_func: Option<Box<dyn FnOnce(&mut Test) + 'func>>,
    pub(crate) teardown_func: Option<Box<dyn FnOnce(&mut Test) + 'func>>,
}

impl<'module,'func> TestBuilder<'module,'func> {
    pub fn new(module: &'module TestModule, name: &'static str) -> Self {
        assert!(!name.contains("::") && !name.contains('/') && !name.contains('.'),
            "Test name should be a single non-delimited token.");

        Self {
            name,
            module,
            using_temp_dir: false,
            inherit_temp_dir: false,
            using_fixture_dir: false,
            inherit_fixture_dir: false,
            setup_func: None,
            teardown_func: None,
        }
    }

    /// Builds the test and initializes it.
    pub fn build(self) -> Test<'module,'func> {
        let namepath = Namepath::new_test(
            self.module.namepath.raw.package_name,
            self.module.use_case,
            self.module.namepath.raw().path,
            self.name)
            .expect("Invalid namepath for Test");

        let temp_dir = if self.using_temp_dir {
            Some(build_temp_dir(&namepath, &self.module.base_temp_dir()))
        } else if self.inherit_temp_dir {
            Some(self.module.temp_dir().to_owned())
        } else {
            None
        };

        let fixture_dir = if self.using_fixture_dir {
            Some(build_fixture_dir(&namepath))
        } else if self.inherit_fixture_dir {
            Some(self.module.fixture_dir().to_owned())
        } else {
            None
        };

        let mut test = Test {
            module: self.module,
            namepath,
            temp_dir,
            fixture_dir,
            teardown_func: self.teardown_func,
        };

        if let Some(setup_fn) = self.setup_func {
            setup_fn(&mut test);
        }

        test
    }

    /// Configures this test to use an existing fixture directory.
    /// The base path is defined by the parent Module or Group, with an existing subdirectory expected to be the name of this test.
    pub fn using_fixture_dir(mut self) -> Self {
        assert!(!self.inherit_fixture_dir, "Configuring both `inherit` and `using` for `fixture_dir` is ambiguous");
        self.using_fixture_dir = true;
        self
    }

    /// Configures the test to use a temporary directory.
    /// The base path is defined by the parent Module or Group, with a subdirectory created just for this test (by its name).
    pub fn using_temp_dir(mut self) -> Self {
        assert!(!self.inherit_temp_dir);
        if self.module.temp_dir.is_none() {
            panic!("Test cannot use a temporary directory unless its parent Module uses one");
        }

        self.using_temp_dir = true;
        self
    }

    /// Configures the test to use the exact same temporary directory as its parent Module or Group.
    /// A separate subdirectory will not be created for this test.
    pub fn inherit_temp_dir(mut self) -> Self {
        assert!(!self.using_temp_dir);
        if self.module.temp_dir.is_none() {
            panic!("Test cannot use a temporary directory unless its parent Module uses one");
        }

        self.inherit_temp_dir = true;
        self
    }

    /// Configures the test to use the exact same fixture directory as its parent Module or Group.
    /// A separate subdirectory for this test is not expected to exist.
    pub fn inherit_fixture_dir(mut self) -> Self {
        assert!(!self.using_fixture_dir);
        self.inherit_fixture_dir = true;
        self
    }

    /// Calls the provided function once on construction of the test.
    pub fn setup(mut self, func: impl FnOnce(&mut Test) + 'func) -> Self {
        self.setup_func = Some(Box::new(func));
        self
    }

    /// Calls the provided function once on destruction of the test.
    pub fn teardown(mut self, func: impl FnOnce(&mut Test) + 'func) -> Self {
        self.teardown_func = Some(Box::new(func));
        self
    }
}

pub struct TestWith<'module, 'func, H> {
    inner: Test<'module, 'func>,
    harness: &'module H,
}

impl<'module, 'func, H> TestWith<'module, 'func, H> {
    pub fn harness(&self) -> &H {
        self.harness
    }
}

impl<'module, 'func, H> Deref for TestWith<'module, 'func, H> {
    type Target = Test<'module, 'func>;
    
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub struct TestBuilderWith<'module, 'func, H> {
    inner: TestBuilder<'module, 'func>,
    harness: &'module H,
}

impl<'module, 'func, H> TestBuilderWith<'module, 'func, H> {
    pub fn new(module: &'module ModuleWith<H>, name: &'static str) -> Self {
        let harness = module.harness();
        let inner = TestBuilder::new(module, name);
        Self { inner, harness }
    }
    
    pub fn using_temp_dir(mut self) -> Self {
        self.inner = self.inner.using_temp_dir();
        self
    }
    
    pub fn inherit_temp_dir(mut self) -> Self {
        self.inner = self.inner.inherit_temp_dir();
        self
    }
    
    pub fn using_fixture_dir(mut self) -> Self {
        self.inner = self.inner.using_fixture_dir();
        self
    }
    
    pub fn inherit_fixture_dir(mut self) -> Self {
        self.inner = self.inner.inherit_fixture_dir();
        self
    }
    
    pub fn teardown(mut self, func: impl FnOnce(&mut Test) + 'func) -> Self {
        self.inner = self.inner.teardown(func);
        self
    }
    
    pub fn build(self) -> TestWith<'module, 'func, H> {
        let inner = self.inner.build();
        let harness = self.harness;
        
        TestWith {
            inner,
            harness,
        }
    }
}

/// Constructs a [Test] using with a parent [TestModule] in scope named, "TESTING".
/// 
/// Requires that the test method is attributed with `#[tested]`.
/// 
/// ## Forms
/// - Bare:
///   - `test!()`
/// - Builder:
///   - `test!({ builder method calls ... })`
/// 
/// ## Examples
/// ### Bare
/// ```rust,ignore
/// let test = testing::test!()
/// ```
/// ### Builder
/// ```rust,ignore
/// let test = testing::test!({
///     .inherit_fixture_dir()
///     .inherit_temp_dir()
/// });
/// ```
#[macro_export]
macro_rules! test {
    ({$($b:tt)+}) => {
        $crate::TestBuilder::new(&TESTING, function_name!())
        $($b)+
            .build()
    };
    () => {
        $crate::TestBuilder::new(&TESTING, function_name!()).build()
    };
}

#[macro_export]
macro_rules! harness_test {
    ({$($b:tt)+}) => {
        $crate::TestBuilderWith::new(&TESTING, function_name!())
        $($b)+
            .build()
    };
    () => {
        $crate::TestBuilderWith::new(&TESTING, function_name!()).build()
    };
}

/// Constructs a [Test] using a custom ident for its parent [TestModule].
#[macro_export]
macro_rules! test_as {
    ($m:ident, {$($b:tt)+}) => {
        let builder = $crate::TestBuilder::new(&$m, function_name!());
        builder$($b)+
            .build()
    };
    ($m:ident) => {
        $crate::TestBuilder::new(&$m, function_name!()).build()
    };
}

impl<'t, 'tm, 'tf> Into<Testable<'t, 'tm, 'tf>> for &'t Test<'tm, 'tf> {
    fn into(self) -> Testable<'t, 'tm, 'tf> {
        self.as_testable()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;

    static MODULE_BASIC: Module = module!(Unit);

    static MODULE_WITH_DIRS: Module = module!(Unit, {
            .using_fixture_dir()
            .using_temp_dir()
    });

    // Test parent Module should be bound.
    #[test] #[named]
    fn test_module() {
        let test = MODULE_BASIC.test_builder(function_name!()).build();
        assert_eq!(&*MODULE_BASIC.namepath(), test.module().namepath(),
            "Test parent Module should be bound.");
    }

    // Test name should be set.
    #[test] #[named]
    fn test_name() {
        let test = MODULE_BASIC.test_builder(function_name!()).build();
        assert_eq!("test_name", test.namepath().name(),
            "Test name should be set.");
    }

    // Test name should not contain namepath separator tokens: "::", '/', '.'
    #[test] #[should_panic]
    fn test_name_invalid() {
        MODULE_BASIC.test_builder("foo.bar").build();  // should panic
    }

    // Test with only a parent Module should have a namepath of: `Test::module().namepath()` / `Test::name()`
    // Test with a parent Group should have a namepath of: `Test::group().namepath()` / `Test::name()`
    #[test] #[named]
    fn test_namepath() {
        const EXPECTED_TEST_NAMEPATH: &'static str = "unit/test/test_namepath";
        let test = MODULE_BASIC.test_builder(function_name!()).build();
        assert_eq!(EXPECTED_TEST_NAMEPATH, test.namepath().to_string());
    }

    // Test not configured with a temp dir should panic when attempting to access it
    #[test] #[should_panic] #[named]
    fn test_temp_dir_unconfigured_access() {
        MODULE_BASIC.test_builder(function_name!())
            .build()
            .temp_dir();  // should panic
    }

    // Test should not allow configuration with `using_temp_dir()` if its parent Module is not using a temp dir.
    #[test] #[should_panic] #[named]
    fn test_temp_dir_using_unconfigured_module() {
        MODULE_BASIC.test_builder(function_name!())
            .using_temp_dir()  // should panic
            .build();
    }

    // Test should not allow configuration with `inherit_temp_dir()` if its parent Module is not using a temp dir.
    #[test] #[should_panic] #[named]
    fn test_temp_dir_inherited_unconfigured_module() {
        MODULE_BASIC.test_builder(function_name!())
            .inherit_temp_dir()  // should panic
            .build();
    }

    // Test configured with `using_tmp_dir()` should have a temp path of: `Module.tmp_dir()` + `Test.name()`
    // Test configured with `using_temp_dir()` should create the directory on construction if it does not exist.
    #[test] #[named]
    fn test_temp_dir_using() {
        let test = MODULE_WITH_DIRS.test_builder(function_name!())
            .using_temp_dir()
            .build();

        assert!(test.temp_dir().exists());
        assert_eq!(MODULE_WITH_DIRS.temp_dir().join("test_temp_dir_using"), test.temp_dir());
    }

    // Test configured to `inherit_temp_dir()` should have the same temp path as its parent.
    #[test] #[named]
    fn test_temp_dir_inherited() {
        let test = MODULE_WITH_DIRS.test_builder(function_name!())
            .inherit_temp_dir()
            .build();

        assert_eq!(MODULE_WITH_DIRS.temp_dir(), test.temp_dir(),
            "Test configured to `inherit_temp_dir()` should have the same temp path as its parent.");
    }

    // Test not configured with a fixture dir should panic when attempting to access it
    #[test] #[should_panic] #[named]
    fn test_fixture_dir_unconfigured_access() {
        MODULE_WITH_DIRS.test_builder(function_name!())
            .build()
            .fixture_dir(); // should panic
    }

    // Test should not allow configuration with `using_fixture_dir()` if its parent Module is not using a fixture dir.
    #[test] #[should_panic] #[named]
    fn test_fixture_dir_using_unconfigured_module() {
        MODULE_BASIC.test_builder(function_name!())
            .using_fixture_dir()  // should panic
            .build();
    }

    // Test should not allow configuration with `inherit_fixture_dir()` if its parent Module is not using a fixture dir.
    #[test] #[should_panic] #[named]
    fn test_fixture_dir_inherited_unconfigured_module() {
        MODULE_BASIC.test_builder(function_name!())
            .inherit_fixture_dir()  // should panic
            .build();
    }

    // Test configured with `using_fixture_dir()` should have a path of: `Module::fixture_dir()` + `Test::name()`
    // Fixture path should exist for Test configured as `using_fixture_dir()` with a parent Module.
    // Test configured with `using_fixture_dir()` should have a path of: `Group::fixture_dir()` + `Test::name()`
    // Fixture path should exist for Test configured as `using_fixture_dir()` with a parent Module.
     #[test] #[named]
    fn test_fixture_dir_using() {
        let test = MODULE_WITH_DIRS.test_builder(function_name!())
            .using_fixture_dir()
            .build();

        assert_eq!(MODULE_WITH_DIRS.fixture_dir().join("test_fixture_dir_using"), test.fixture_dir(),
            "Test configured with `using_fixture_dir()` should have a path of: `Module::fixture_dir()` + `Test::name()`");
        assert!(test.fixture_dir().exists(),
            "Fixture path should exist for Test configured as `using_fixture_dir()`");
    }

    // Test configured to `inherit_fixture_dir()` should have a fixture path that is the same as its Module.
    // Fixture path should exist for Test configured to `inherit_fixture_dir()` from Module
    // Test configured to `inherit_fixture_dir()` should have a fixture path that is the same as its Group.
    // Fixture path should exist for Test configured to `inherit_fixture_dir()` from Group
    #[test] #[named]
    fn test_fixture_dir_inherited() {
        let test = MODULE_WITH_DIRS.test_builder(function_name!())
            .inherit_fixture_dir()
            .build();

        assert_eq!(MODULE_WITH_DIRS.fixture_dir(), test.fixture_dir(),
            "Test configured to `inherit_fixture_dir()` should have a fixture path that is the same as its Module.");
        assert!(test.fixture_dir().exists(),
            "Fixture path should exist for Test configured to `inherit_fixture_dir()` from Module");
    }

    // SAFETY: This can only be called once, by `test_setup_function()`. Not thread safe.
    static mut SETUP_FUNC_CALLED: bool = false;
    fn setup_func(_test: &mut Test) {
        unsafe {
            SETUP_FUNC_CALLED = true;
        }
    }

    // Test setup function should be ran on construction.
    #[test] #[named]
    fn test_setup_function() {
        let _testgroup = MODULE_BASIC.test_builder(function_name!())
            .setup(setup_func)
            .build();

        unsafe {
            assert!(SETUP_FUNC_CALLED,
                "Test setup function should be ran on construction.");
        }
    }

    // Test setup closure should be ran on construction.
    #[test] #[named]
    fn test_setup_closure() {
        let mut setup_closure_called = false;
        MODULE_BASIC.test_builder(function_name!())
            .setup(|_| {
                setup_closure_called = true;
            })
            .build();

        assert!(setup_closure_called,
            "Test setup closure should be ran on construction.");
    }

    // unsafe: This can only be called once, by `test_setup_function()`. Not thread safe.
    static mut TEARDOWN_FUNC_CALLED: bool = false;
    fn teardown_func(_group: &mut Test) {
        unsafe {
            TEARDOWN_FUNC_CALLED = true;
        }
    }

    // Test teardown function should be ran on destruction.
    #[test] #[named]
    fn test_teardown_function() {
        {
            MODULE_BASIC.test_builder(function_name!())
            .teardown(teardown_func)
            .build();
        }

        unsafe {
            assert!(TEARDOWN_FUNC_CALLED,
                "Test teardown function should be ran on destruction.");
        }
    }

    // Test teardown closure should be ran on destruction.
    #[test] #[named]
    fn test_teardown_closure() {
        let mut teardown_closure_called = false;
        {
            MODULE_BASIC.test_builder(function_name!())
                .teardown(|_| {
                    teardown_closure_called = true;
                })
                .build();
        }

        assert!(teardown_closure_called,
            "Test teardown closure should be ran on destruction.");
    }
}
