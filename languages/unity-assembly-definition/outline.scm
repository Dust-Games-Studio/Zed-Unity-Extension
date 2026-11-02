(document
  (object
    (pair
      key: (string (string_content) @name)) @item))

(array
  (object
    (pair
      key: (string (string_content) @_key)
      (#eq? @_key "name")
      value: (string (string_content) @name)) @item))
