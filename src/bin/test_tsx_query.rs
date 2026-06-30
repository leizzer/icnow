use tree_sitter::{Parser, Query, QueryCursor};

fn main() {
    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_typescript::LANGUAGE_TSX.into()).unwrap();

    let source_code = r#"
        import React, { useState, useEffect } from 'react';
        import { useHostContext } from '../contexts/HostContext';
        
        export const MyComponent = () => {
            const host = useHostContext();
            return <div>{host}</div>;
        };

        export function MyFunc() {}
        export interface MyInterface {}
        export type MyType = string;
        
        const internalFunc = () => {};
    "#;

    let tree = parser.parse(source_code, None).unwrap();
    
    println!("{}", tree.root_node().to_sexp());
}
