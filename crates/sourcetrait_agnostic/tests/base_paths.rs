//! The system's base paths, each kind answering its own directory.
use sourcetrait_agnostic::{self as agnostic, prelude::*};

#[test]
fn each_base_path_answers_its_own_directory() {
    let path = agnostic::PLATFORM.path();
    let cache = path.dir(agnostic::Dir::HomeCache).expect("the cache dir");
    let config = path.dir(agnostic::Dir::HomeConfig).expect("the config dir");
    let data = path.dir(agnostic::Dir::HomeData).expect("the data dir");
    assert_ne!(config, cache, "the environment keeps config and cache apart");
    assert_ne!(data, cache, "the environment keeps data and cache apart");
    let base = agnostic::BasePaths::sys();
    assert_eq!(base.cache_dir(), cache);
    assert_eq!(base.config_dir(), config);
    assert_eq!(base.data_dir(), data);
}
