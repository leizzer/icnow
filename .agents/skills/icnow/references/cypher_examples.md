# Cypher Query Examples (`query_graph_cypher`)

When using `query_graph_cypher`, remember that nodes are either `Symbol` or `File`. `Symbol` nodes have a `kind` property (e.g., `'Method'`, `'Class'`, `'Macro'`, `'Variable'`, `'Import'`).

**Example 1: Count all methods inside a specific file**
```cypher
MATCH (f:File {id: 'app/models/user.rb'})-[:CONTAINS]->(m:Symbol {kind: 'Method'})
RETURN count(m)
```

**Example 2: Find all classes that inherit from `ApplicationRecord`**
```cypher
MATCH (c:Symbol {kind: 'Class'})-[:CALLS]->(p:Symbol {name: 'ApplicationRecord'})
RETURN c.id, c.name
```

**Example 3: Find all files that import a specific module**
```cypher
MATCH (f:File)-[:IMPORTS]->(i:Symbol {name: 'react'})
RETURN f.id
```
