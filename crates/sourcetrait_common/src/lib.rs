#[cfg(feature = "agnostic")]
pub mod agnostic {
    pub use sourcetrait_agnostic::*;
    pub mod bridge {
        pub use sourcetrait_agnostic_bridge::*;
    }
}
#[cfg(feature = "cereal")]
pub mod cereal {
    pub use sourcetrait_cereal::*;
    pub use sourcetrait_cereal_macro::*;
}
#[cfg(feature = "chronox")]
pub mod chronox {
    pub use sourcetrait_cereal::*;
}
#[cfg(feature = "clapx")]
pub mod clapx {
    pub use sourcetrait_clapx::*;
}
#[cfg(feature = "datum")]
pub mod datum {
    pub use sourcetrait_datum::*;
}
#[cfg(feature = "dotrepo")]
pub mod dotrepo {
    pub use sourcetrait_dotrepo::*;
}
#[cfg(feature = "gitparity")]
pub mod gitparity {
    pub use sourcetrait_gitparity::*;
}
#[cfg(feature = "ronx")]
pub mod ronx {
    pub use sourcetrait_ronx::*;
    pub mod macros {
        pub use sourcetrait_ronx_macro::*;
    }
}
#[cfg(feature = "stdx")]
pub mod stdx {
    pub use sourcetrait_stdx::*;
}
#[cfg(feature = "subsys")]
pub mod subsys {
    pub use sourcetrait_subsys::*;
}
#[cfg(feature = "testing")]
pub mod testing {
    pub use sourcetrait_testing::*;
    pub mod macros {
        pub use sourcetrait_testing_macro::*;
    }
}
#[cfg(feature = "tomlx")]
pub mod tomlx {
    pub use sourcetrait_tomlx::*;
}
#[cfg(feature = "tooling")]
pub mod tooling {
    pub use sourcetrait_tooling::*;
}
#[cfg(feature = "twostr")]
pub mod twostr {
    pub use sourcetrait_twostr::*;
}

pub mod prelude {
    #[cfg(feature = "agnostic")]
    pub use sourcetrait_agnostic::prelude::*;
    #[cfg(feature = "agnostic")]
    pub use sourcetrait_agnostic_bridge::prelude::*;
    #[cfg(feature = "cereal")]
    pub use sourcetrait_cereal::prelude::*;
    #[cfg(feature = "gitparity")]
    pub use sourcetrait_gitparity::prelude::*;
    #[cfg(feature = "ronx")]
    pub use sourcetrait_ronx::prelude::*;
    #[cfg(feature = "stdx")]
    pub use sourcetrait_stdx::*;
    #[cfg(feature = "subsys")]
    pub use sourcetrait_subsys::prelude::*;
    #[cfg(feature = "testing")]
    pub use sourcetrait_testing::prelude::*;
    #[cfg(feature = "tomlx")]
    pub use sourcetrait_tomlx::prelude::*;
    #[cfg(feature = "twostr")]
    pub use sourcetrait_twostr::prelude::*;
}
