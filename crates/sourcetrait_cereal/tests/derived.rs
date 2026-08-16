#[cfg(test)]
mod tests {
    use sourcetrait_cereal as cereal_lib;
    use sourcetrait_cereal_macro as cereal;
    
    #[test]
    fn test_derive_default() {
        #[cereal::derived]
        struct TestData {
            a: u32,
        }
    }
    
    #[test]
    fn test_derive_eq() {
        #[cereal::derived(Eq)]
        struct TestData {
            a: u32,
        }
        
        let expected = TestData { a: 5 };
        let actual = TestData { a: 5 };
        
        fn assert_eq_tag<T: Eq + std::fmt::Debug>(a: &T, b: &T) { assert_eq!(a, b) }
        assert_eq_tag(&expected, &actual);
    }
    
    #[test]
    fn test_derive_copy() {
        #[cereal::derived(Copy)]
        struct TestData {
            a: u32,
        }
        
        let a = TestData { a: 5 };
        let b = a; // copy here, instead of move
        assert_eq!(a, b);
    }
    
    #[test]
    fn test_derive_has_debug() {
        #[cereal::derived(has(Debug))]
        struct TestData {
            a: u32,
        }
        
        impl std::fmt::Debug for TestData {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "TestData {{ a: {} }}", self.a)
            }
        }
        
        let expected = TestData { a: 5 };
        let actual = TestData { a: 5 };
        
        assert_eq!(expected, actual);
    }
    
    #[test]
    fn test_data_trait() {
        #[cereal::derived(Data)]
        struct TestData {
            a: u32,
        }
        
        let a = TestData { a: 10 };
        let b = TestData { a: 10 };
        
        fn assert_eq_data<T: cereal_lib::Data>(a: &T, b: &T) { assert_eq!(a, b) }
        assert_eq_data(&a, &b);
    }
}
        