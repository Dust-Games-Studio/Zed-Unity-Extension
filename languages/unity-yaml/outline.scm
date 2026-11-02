(document
  (block_node
    (tag)? @context
    (anchor)? @context
    (block_mapping
      (block_mapping_pair
        key: (flow_node
          (plain_scalar
            (string_scalar) @name)) @item))))

(block_mapping_pair
  key: (flow_node
    (plain_scalar
      (string_scalar) @_key))
  (#eq? @_key "m_Name")
  value: (flow_node
    (plain_scalar
      (string_scalar) @name))) @item
