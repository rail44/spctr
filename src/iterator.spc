(target) => {
  to_list: reduce([], (list, el) => list.concat([el])),

  map: (fn) => {
    inner: (target) => {
      next: {
        if target.next = null
          null

        [
          inner(target.next[0]),
          fn(target.next[1])
        ]
      }
    },

    Iterator(inner(target))
  },

  reduce: (initial, fn) => {
    inner: (target, acc) => {
      if target.next = null
        acc

      inner(target.next[0], fn(acc, target.next[1]))
    },

    inner(target, initial)
  },

  find: (fn) => {
    inner: (target) => {
      if target.next = null
        null

      if fn(target.next[1])
        target.next[1]

      inner(target.next[0])
    },

    inner(target)
  },

  filter: (fn) => {
    inner: (target) => {
      next_iter: inner(target.next[0]),
      next: {
        if target.next = null
          null

        if fn(target.next[1])
          next_iter.next

        [
          next_iter,
          target.next[1]
        ]
      }
    },

    Iterator(inner(target))
  }
}
