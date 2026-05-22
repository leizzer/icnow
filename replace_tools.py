import os

path = "/Users/cristian/Projects/blackhole/icnow/src/tools.rs"
with open(path, "r") as f:
    content = f.read()

# Replace traverse_graph
old_traverse = """    #[tool(description = "Recursively walks the graph bidirectionally from a starting node up to a specified depth (max_depth) and returns an indented relationship list. Use this tool when you want to discover the neighborhood of dependencies, callers, or subclasses of a particular node in a single call.")]
    fn traverse_graph(&self, Parameters(req): Parameters<TraverseGraphRequest>) -> Result<String, String> {
        let depth = req.max_depth.unwrap_or(2);
        
        let query = format!(
            "WITH RECURSIVE graph_path(source_int, target_int, label, depth) AS ( \\
                SELECT source_id, target_id, type, 1 \\
                FROM edges \\
                WHERE source_id = ( \\
                    SELECT np.node_id FROM node_props_text np \\
                    JOIN property_keys pk ON np.key_id = pk.id \\
                    WHERE pk.key = 'id' AND np.value = '{node_id}' \\
                ) OR target_id = ( \\
                    SELECT np.node_id FROM node_props_text np \\
                    JOIN property_keys pk ON np.key_id = pk.id \\
                    WHERE pk.key = 'id' AND np.value = '{node_id}' \\
                ) \\
                UNION \\
                SELECT e.source_id, e.target_id, e.type, gp.depth + 1 \\
                FROM edges e \\
                JOIN graph_path gp ON e.source_id = gp.target_int OR e.target_id = gp.source_int OR e.source_id = gp.source_int OR e.target_id = gp.target_int \\
                WHERE gp.depth < {depth} \\
            ) \\
            SELECT DISTINCT \\
                (SELECT np.value FROM node_props_text np JOIN property_keys pk ON np.key_id = pk.id WHERE pk.key = 'id' AND np.node_id = gp.source_int) as source, \\
                (SELECT np.value FROM node_props_text np JOIN property_keys pk ON np.key_id = pk.id WHERE pk.key = 'id' AND np.node_id = gp.target_int) as target, \\
                gp.label, \\
                gp.depth \\
            FROM graph_path gp ORDER BY gp.depth ASC;",
            node_id = req.node_id,
            depth = depth
        );
        
        let output = std::process::Command::new("sqlite3")
            .arg(&self.db_path)
            .arg("-header")
            .arg("-markdown")
            .arg(&query)
            .output()
            .map_err(|e| format!("Failed to execute traversal: {}", e))?;
            
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Traversal failed: {}", err));
        }
        
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }"""

new_traverse = """    #[tool(description = "Recursively walks the graph bidirectionally from a starting node up to a specified depth (max_depth) and returns an indented relationship list. Use this tool when you want to discover the neighborhood of dependencies, callers, or subclasses of a particular node in a single call.")]
    fn traverse_graph(&self, Parameters(req): Parameters<TraverseGraphRequest>) -> Result<String, String> {
        let depth = req.max_depth.unwrap_or(2);
        let safe_node_id = req.node_id.replace("'", "''");
        
        let query = if depth <= 1 {
            format!(
                "MATCH (n) WHERE n.id = '{id}' MATCH (n)-[r]->(y) RETURN n.id as source, type(r) as label, y.id as target \\
                 UNION \\
                 MATCH (n) WHERE n.id = '{id}' MATCH (y)-[r]->(n) RETURN y.id as source, type(r) as label, n.id as target",
                id = safe_node_id
            )
        } else {
            let depth_minus_1 = depth - 1;
            format!(
                "MATCH (n) WHERE n.id = '{id}' MATCH (n)-[r]->(y) RETURN n.id as source, type(r) as label, y.id as target \\
                 UNION \\
                 MATCH (n) WHERE n.id = '{id}' MATCH (y)-[r]->(n) RETURN y.id as source, type(r) as label, n.id as target \\
                 UNION \\
                 MATCH (n)-[*1..{d}]->(x) WHERE n.id = '{id}' MATCH (x)-[r]->(y) RETURN x.id as source, type(r) as label, y.id as target \\
                 UNION \\
                 MATCH (n)-[*1..{d}]->(x) WHERE n.id = '{id}' MATCH (y)-[r]->(x) RETURN y.id as source, type(r) as label, x.id as target \\
                 UNION \\
                 MATCH (n)<-[*1..{d}]-(x) WHERE n.id = '{id}' MATCH (x)-[r]->(y) RETURN x.id as source, type(r) as label, y.id as target \\
                 UNION \\
                 MATCH (n)<-[*1..{d}]-(x) WHERE n.id = '{id}' MATCH (y)-[r]->(x) RETURN y.id as source, type(r) as label, x.id as target",
                id = safe_node_id, d = depth_minus_1
            )
        };
        
        let conn = graphqlite::Connection::open(&self.db_path)
            .map_err(|e| format!("Failed to open graph database: {}", e))?;
            
        let res = conn.cypher(&query)
            .map_err(|e| format!("Cypher query failed: {}", e))?;
            
        format_cypher_result(res)
    }"""

# Replace query_graph_cypher
old_cypher = """    #[tool(description = "Executes a graph query using Cypher syntax (e.g., MATCH (source)-[rel]->(target) WHERE ...) to discover patterns, links, or cross-file dependencies. This is the preferred tool for high-level semantic lookups and pattern matching in the database.")]
    fn query_graph_cypher(&self, Parameters(req): Parameters<QueryGraphCypherRequest>) -> Result<String, String> {
        let conn = graphqlite::Connection::open(&self.db_path)
            .map_err(|e| format!("Failed to open graph database: {}", e))?;
            
        let res = conn.cypher(&req.query)
            .map_err(|e| format!("Cypher query failed: {}", e))?;
            
        let cols = res.columns();
        if cols.is_empty() {
            return Ok("No columns returned.".to_string());
        }
        
        let mut out = format!("| {} |\\n", cols.join(" | "));
        out.push_str(&format!("| {} |\\n", cols.iter().map(|_| "---").collect::<Vec<_>>().join(" | ")));
        
        for row in &res {
            let mut row_vals = Vec::new();
            for col in cols {
                let val_str = if let Ok(s) = row.get::<String>(col) {
                    s
                } else if let Ok(i) = row.get::<i64>(col) {
                    i.to_string()
                } else if let Ok(f) = row.get::<f64>(col) {
                    f.to_string()
                } else if let Ok(b) = row.get::<bool>(col) {
                    b.to_string()
                } else {
                    "null".to_string()
                };
                row_vals.push(val_str);
            }
            out.push_str(&format!("| {} |\\n", row_vals.join(" | ")));
        }
        
        Ok(out)
    }"""

new_cypher = """    #[tool(description = "Executes a graph query using Cypher syntax (e.g., MATCH (source)-[rel]->(target) WHERE ...) to discover patterns, links, or cross-file dependencies. This is the preferred tool for high-level semantic lookups and pattern matching in the database.")]
    fn query_graph_cypher(&self, Parameters(req): Parameters<QueryGraphCypherRequest>) -> Result<String, String> {
        let conn = graphqlite::Connection::open(&self.db_path)
            .map_err(|e| format!("Failed to open graph database: {}", e))?;
            
        let res = conn.cypher(&req.query)
            .map_err(|e| format!("Cypher query failed: {}", e))?;
            
        format_cypher_result(res)
    }"""

# Replace search_symbols
old_search = """    #[tool(description = "Searches the graph for nodes matching a symbol name or pattern (e.g., a class, function, or file name). Use this tool to instantly find where a symbol is defined across the entire workspace without knowing its file path.")]
    fn search_symbols(&self, Parameters(req): Parameters<SearchSymbolsRequest>) -> Result<String, String> {
        let limit = req.limit.unwrap_or(50);
        let safe_query = req.query.replace("'", "''");
        
        let sql = format!(
            "SELECT np_id.value AS id, nl.label \\
             FROM nodes n \\
             JOIN node_props_text np_id ON n.id = np_id.node_id \\
             JOIN property_keys pk_id ON np_id.key_id = pk_id.id AND pk_id.key = 'id' \\
             LEFT JOIN node_labels nl ON n.id = nl.node_id \\
             WHERE np_id.value LIKE '%{}%' \\
             LIMIT {};",
            safe_query, limit
        );
        
        let output = std::process::Command::new("sqlite3")
            .arg("-cmd")
            .arg(".timeout 5000")
            .arg(&self.db_path)
            .arg("-header")
            .arg("-markdown")
            .arg(&sql)
            .output()
            .map_err(|e| format!("Failed to execute query: {}", e))?;
            
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Query failed: {}", err));
        }
        
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }"""

new_search = """    #[tool(description = "Searches the graph for nodes matching a symbol name or pattern (e.g., a class, function, or file name). Use this tool to instantly find where a symbol is defined across the entire workspace without knowing its file path.")]
    fn search_symbols(&self, Parameters(req): Parameters<SearchSymbolsRequest>) -> Result<String, String> {
        let limit = req.limit.unwrap_or(50);
        let safe_query = req.query.replace("'", "''");
        
        let cypher_query = format!(
            "MATCH (n) WHERE toLower(n.id) CONTAINS toLower('{}') RETURN n.id as id, labels(n) as label LIMIT {}",
            safe_query, limit
        );
        
        let conn = graphqlite::Connection::open(&self.db_path)
            .map_err(|e| format!("Failed to open graph database: {}", e))?;
            
        let res = conn.cypher(&cypher_query)
            .map_err(|e| format!("Cypher query failed: {}", e))?;
            
        format_cypher_result(res)
    }"""

# Replace get_dependencies and add format_cypher_result
old_deps = """    #[tool(description = "Traces incoming or outgoing references for a specific node ID (e.g. finding callers or callees of a function). Provide the node_id and the direction ('incoming' for callers, 'outgoing' for callees).")]
    fn get_dependencies(&self, Parameters(req): Parameters<GetDependenciesRequest>) -> Result<String, String> {
        let safe_node_id = req.node_id.replace("'", "''");
        let sql = if req.direction == "incoming" {
            format!(
                "SELECT np_source.value AS caller, e.type AS relationship \\
                 FROM edges e \\
                 JOIN node_props_text np_target ON e.target_id = np_target.node_id \\
                 JOIN property_keys pk_target ON np_target.key_id = pk_target.id AND pk_target.key = 'id' \\
                 JOIN node_props_text np_source ON e.source_id = np_source.node_id \\
                 JOIN property_keys pk_source ON np_source.key_id = pk_source.id AND pk_source.key = 'id' \\
                 WHERE np_target.value LIKE '%{}';",
                safe_node_id
            )
        } else {
            format!(
                "SELECT e.type AS relationship, np_target.value AS callee \\
                 FROM edges e \\
                 JOIN node_props_text np_source ON e.source_id = np_source.node_id \\
                 JOIN property_keys pk_source ON np_source.key_id = pk_source.id AND pk_source.key = 'id' \\
                 JOIN node_props_text np_target ON e.target_id = np_target.node_id \\
                 JOIN property_keys pk_target ON np_target.key_id = pk_target.id AND pk_target.key = 'id' \\
                 WHERE np_source.value LIKE '%{}';",
                safe_node_id
            )
        };
        
        let output = std::process::Command::new("sqlite3")
            .arg("-cmd")
            .arg(".timeout 5000")
            .arg(&self.db_path)
            .arg("-header")
            .arg("-markdown")
            .arg(&sql)
            .output()
            .map_err(|e| format!("Failed to execute query: {}", e))?;
            
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Query failed: {}", err));
        }
        
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}"""

new_deps = """    #[tool(description = "Traces incoming or outgoing references for a specific node ID (e.g. finding callers or callees of a function). Provide the node_id and the direction ('incoming' for callers, 'outgoing' for callees).")]
    fn get_dependencies(&self, Parameters(req): Parameters<GetDependenciesRequest>) -> Result<String, String> {
        let safe_node_id = req.node_id.replace("'", "''");
        
        let cypher_query = if req.direction == "incoming" {
            format!(
                "MATCH (source)-[r]->(target) WHERE toLower(target.id) ENDS WITH toLower('{}') OR toLower(target.id) = toLower('{}') RETURN source.id as caller, type(r) as relationship",
                safe_node_id, safe_node_id
            )
        } else {
            format!(
                "MATCH (source)-[r]->(target) WHERE toLower(source.id) ENDS WITH toLower('{}') OR toLower(source.id) = toLower('{}') RETURN type(r) as relationship, target.id as callee",
                safe_node_id, safe_node_id
            )
        };
        
        let conn = graphqlite::Connection::open(&self.db_path)
            .map_err(|e| format!("Failed to open graph database: {}", e))?;
            
        let res = conn.cypher(&cypher_query)
            .map_err(|e| format!("Cypher query failed: {}", e))?;
            
        format_cypher_result(res)
    }
}

fn format_cypher_result(res: graphqlite::CypherResult) -> Result<String, String> {
    let cols = res.columns();
    if cols.is_empty() {
        return Ok("No columns returned.".to_string());
    }
    
    let mut out = format!("| {} |\\n", cols.join(" | "));
    out.push_str(&format!("| {} |\\n", cols.iter().map(|_| "---").collect::<Vec<_>>().join(" | ")));
    
    for row in &res {
        let mut row_vals = Vec::new();
        for col in cols {
            let val_str = if let Ok(s) = row.get::<String>(col) {
                s
            } else if let Ok(i) = row.get::<i64>(col) {
                i.to_string()
            } else if let Ok(f) = row.get::<f64>(col) {
                f.to_string()
            } else if let Ok(b) = row.get::<bool>(col) {
                b.to_string()
            } else {
                "null".to_string()
            };
            row_vals.push(val_str);
        }
        out.push_str(&format!("| {} |\\n", row_vals.join(" | ")));
    }
    
    Ok(out)
}"""

content = content.replace(old_traverse, new_traverse)
content = content.replace(old_cypher, new_cypher)
content = content.replace(old_search, new_search)
content = content.replace(old_deps, new_deps)

with open(path, "w") as f:
    f.write(content)

print("Done replacing.")
