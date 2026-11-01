use crate::*;

/// Windows, each component its driver's lookup as on Linux and macOS. It
/// claims no capability yet: its account lookups await the driver's SID
/// lookups, and until then each answers incapable.
pub struct WindowsCrossPlatform;
impl CrossPlatform for WindowsCrossPlatform {
    const OS: Os = Os::Windows;
    const OS_FAMILY: OsFamily = OsFamily::Windows;

    #[inline]
    fn capabilities(&self) -> Capabilities {
        Capabilities::NONE
    }

    #[inline]
    fn access(&self) -> impl AccessComponentTrait {
        StandardAccessComponent(windows::WindowsAccessComponentLookup)
    }

    #[inline]
    fn cmd(&self) -> impl CmdComponentTrait {
        StandardCmdComponent(windows::WindowsCmdComponentLookup)
    }

    #[inline]
    fn fs(&self) -> impl FilesComponentTrait {
        StandardFilesComponent(windows::WindowsFilesComponentLookup)
    }

    #[inline]
    fn net(&self) -> impl NetComponentTrait {
        StandardNetComponent(windows::WindowsNetComponentLookup)
    }

    #[inline]
    fn path(&self) -> impl PathsComponentTrait {
        StandardPathsComponent(windows::WindowsPathsComponentLookup)
    }

    #[inline]
    fn ui(&self) -> impl UiComponentTrait {
        StandardUiComponent(windows::WindowsUiComponentLookup)
    }
}
