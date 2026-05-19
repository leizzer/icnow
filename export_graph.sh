#!/bin/bash

if [ -z "$1" ]; then
    echo "Usage: $0 <database.db> [output.png]"
    exit 1
fi

DB=$1
OUT=${2:-output.png}
DOT_FILE="temp_graph.dot"

echo "digraph G {" > $DOT_FILE
echo "  node [shape=box, style=filled, fillcolor=lightblue];" >> $DOT_FILE

# Output nodes
# We join nodes with node_labels and node_props_text to get a friendly display label.
# graphqlite stores the original ID provided to upsert_node as a property key 'id'.
sqlite3 "$DB" "
SELECT 
  n.id || ' [label=\"' || 
  COALESCE(nl.label, 'Node') || '\\n' || 
  'ID: ' || COALESCE(MAX(CASE WHEN pk.key = 'id' THEN npt.value END), n.id) || 
  COALESCE('\\n' || GROUP_CONCAT(CASE WHEN pk.key != 'id' AND pk.key != 'source_code' THEN pk.key || ': ' || npt.value END, '\\n'), '') ||
  '\"];'
FROM nodes n
LEFT JOIN node_labels nl ON nl.node_id = n.id
LEFT JOIN node_props_text npt ON npt.node_id = n.id
LEFT JOIN property_keys pk ON pk.id = npt.key_id
GROUP BY n.id, nl.label;
" >> $DOT_FILE

# Output edges
sqlite3 "$DB" "
SELECT source_id || ' -> ' || target_id || ' [label=\"' || type || '\"];' FROM edges;
" >> $DOT_FILE

echo "}" >> $DOT_FILE

# Render to PNG using Graphviz
dot -Tpng $DOT_FILE -o "$OUT"

if [ $? -eq 0 ]; then
    echo "Graph successfully exported to $OUT"
    rm $DOT_FILE
else
    echo "Error generating PNG. Do you have graphviz installed?"
fi
