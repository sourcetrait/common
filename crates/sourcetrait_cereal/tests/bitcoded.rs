//! cereal's recursive types: their round trips through bitcode's lazy coders,
//! their wire held to bitcode's own derive over the same shape unrolled by
//! level, a level with no values emitting and reading nothing, a budgeted
//! decode bounding their depth, and rkyv over them.
use bitcode::coder::EncoderBuffer;
use bitcode::{Budget, BudgetError, Encode, Exceeded};
use std::num::NonZeroUsize;

/// The `cereal` path the attribute and its output are named through, as
/// sourcetrait_common gives it.
mod cereal {
    pub use sourcetrait_cereal::*;
    pub use sourcetrait_cereal_macro::*;
}

/// A tree, recursive through its children.
#[cereal::derived(Data, Eq)]
enum Tree {
    Leaf(u8),
    Node(Vec<Tree>),
}

/// A list, recursive through `Self`.
#[cereal::derived(Data, Eq)]
struct List {
    head: u32,
    tail: Option<Box<Self>>,
}

/// A generic tree, its fields named.
#[cereal::derived(Data, Eq)]
struct Branch<T> {
    value: T,
    pair: (u16, String),
    children: Vec<Branch<T>>,
}

/// A recursive tuple struct.
#[cereal::derived(Data, Eq)]
struct Nest(u8, Vec<Nest>);

/// Each tree's shape, three levels deep at most.
fn trees() -> Vec<Tree> {
    use Tree::{Leaf, Node};
    vec![
        Leaf(7),
        Node(vec![]),
        Node(vec![Leaf(1), Node(vec![])]),
        Node(vec![Node(vec![Leaf(2), Leaf(3)]), Leaf(4), Node(vec![Node(vec![])])]),
        Node(vec![Node(vec![]), Node(vec![Node(vec![]), Leaf(5)])]),
    ]
}

/// A chain of `depth` nodes around a leaf.
fn chain(depth: usize) -> Tree {
    (0..depth).fold(Tree::Leaf(0), |tree, _| Tree::Node(vec![tree]))
}

#[test]
fn a_recursive_type_round_trips() {
    for tree in trees().into_iter().chain([chain(60)]) {
        assert_eq!(bitcode::decode::<Tree>(&bitcode::encode(&tree)).unwrap(), tree);
    }
    let all = trees();
    assert_eq!(bitcode::decode::<Vec<Tree>>(&bitcode::encode(&all)).unwrap(), all);

    let list = List {
        head: 1,
        tail: Some(Box::new(List {
            head: 2,
            tail: Some(Box::new(List { head: 3, tail: None })),
        })),
    };
    assert_eq!(bitcode::decode::<List>(&bitcode::encode(&list)).unwrap(), list);

    let branch = Branch {
        value: 9u64,
        pair: (300, "root".to_owned()),
        children: vec![
            Branch {
                value: 1,
                pair: (1, String::new()),
                children: vec![],
            },
            Branch {
                value: 2,
                pair: (2, "two".to_owned()),
                children: vec![Branch {
                    value: 3,
                    pair: (3, "three".to_owned()),
                    children: vec![],
                }],
            },
        ],
    };
    assert_eq!(bitcode::decode::<Branch<u64>>(&bitcode::encode(&branch)).unwrap(), branch);

    let nest = Nest(1, vec![Nest(2, vec![]), Nest(3, vec![Nest(4, vec![])])]);
    assert_eq!(bitcode::decode::<Nest>(&bitcode::encode(&nest)).unwrap(), nest);

    let mut buffer = bitcode::Buffer::new();
    for tree in trees() {
        let bytes = bitcode::encode(&tree);
        assert_eq!(buffer.encode(&tree), bytes.as_slice());
        assert_eq!(buffer.decode::<Tree>(&bytes).unwrap(), tree);
    }
}

/// `Tree` unrolled three levels deep, each level bitcode's own derive.
#[derive(bitcode::Encode, bitcode::Decode, Debug, PartialEq)]
enum Tree0 {
    Leaf(u8),
    Node(Vec<Tree1>),
}

#[derive(bitcode::Encode, bitcode::Decode, Debug, PartialEq)]
enum Tree1 {
    Leaf(u8),
    Node(Vec<Tree2>),
}

#[derive(bitcode::Encode, bitcode::Decode, Debug, PartialEq)]
enum Tree2 {
    Leaf(u8),
    Node(Vec<Bottom>),
}

/// The level below the deepest, which holds no value.
#[derive(bitcode::Encode, bitcode::Decode, Debug, PartialEq)]
enum Bottom {}

fn unroll0(tree: &Tree) -> Tree0 {
    match tree {
        Tree::Leaf(v) => Tree0::Leaf(*v),
        Tree::Node(children) => Tree0::Node(children.iter().map(unroll1).collect()),
    }
}

fn unroll1(tree: &Tree) -> Tree1 {
    match tree {
        Tree::Leaf(v) => Tree1::Leaf(*v),
        Tree::Node(children) => Tree1::Node(children.iter().map(unroll2).collect()),
    }
}

fn unroll2(tree: &Tree) -> Tree2 {
    match tree {
        Tree::Leaf(v) => Tree2::Leaf(*v),
        Tree::Node(children) => {
            assert!(children.is_empty(), "three levels at most");
            Tree2::Node(Vec::new())
        }
    }
}

#[test]
fn each_level_takes_the_columns_bitcode_s_derive_gives_it() {
    for tree in trees() {
        let unrolled = unroll0(&tree);
        let bytes = bitcode::encode(&tree);
        assert_eq!(bytes, bitcode::encode(&unrolled), "{tree:?}");
        assert_eq!(bitcode::decode::<Tree0>(&bytes).unwrap(), unrolled);
    }
    let all = trees();
    let unrolled: Vec<Tree0> = all.iter().map(unroll0).collect();
    assert_eq!(bitcode::encode(&all), bitcode::encode(&unrolled));
}

#[test]
fn a_level_with_no_values_emits_and_reads_nothing() {
    let mut encoder = <Tree as Encode>::Encoder::default();
    encoder.reserve(NonZeroUsize::new(3).unwrap());
    assert!(encoder.collect().is_empty());

    // An option encoder reserves its present values' encoder for a chunk and
    // encodes none of them here.
    let absent = vec![None::<Tree>; 5];
    let bytes = bitcode::encode(&absent);
    assert_eq!(bytes, bitcode::encode(&vec![None::<()>; 5]));
    assert_eq!(bitcode::decode::<Vec<Option<Tree>>>(&bytes).unwrap(), absent);
}

#[test]
fn a_budgeted_decode_bounds_a_recursive_type_s_depth() {
    let open = Budget {
        elements: u64::MAX,
        heap: u64::MAX,
        depth: u32::MAX,
    };
    let deep = chain(40);
    let bytes = bitcode::encode(&deep);
    assert_eq!(bitcode::decode_budgeted::<Tree>(&bytes, open).unwrap(), deep);
    // Each level's children are a collection, which takes a level of depth.
    let shallow = Budget { depth: 39, ..open };
    assert!(matches!(
        bitcode::decode_budgeted::<Tree>(&bytes, shallow),
        Err(BudgetError::Exceeded(Exceeded::Depth))
    ));
    assert_eq!(
        bitcode::decode_budgeted::<Tree>(&bytes, Budget { depth: 40, ..open }).unwrap(),
        deep
    );
}

#[test]
fn rkyv_takes_a_recursive_type() {
    for tree in trees().into_iter().chain([chain(20)]) {
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&tree).unwrap();
        let back = rkyv::from_bytes::<Tree, rkyv::rancor::Error>(&bytes).unwrap();
        assert_eq!(back, tree);
    }
    let nest = Nest(1, vec![Nest(2, vec![Nest(3, vec![])])]);
    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&nest).unwrap();
    assert_eq!(rkyv::from_bytes::<Nest, rkyv::rancor::Error>(&bytes).unwrap(), nest);
}
