Iterator: {
  range: (from, to) => {
    inner: (i) => () => {
      if i = to {
        null
      }

      [
        inner(i + 1),
        i
      ]
    },

    Iterator.new(inner(from))
  },

  new: (target) => {
    to_list: reduce([], (list, el) => List.concat(list, [el])),

    map: (fn) => {
      inner: (target) => {
        next: target(),
        () => {
          if next = null
              null

          [
            inner(next[0]),
            fn(next[1])
          ]
        }
      },

      Iterator.new(inner(target))
    },

    reduce: (initial, fn) => {
      inner: (target, acc) => {
        next: target(),

        if next = null
          acc

        inner(next[0], fn(acc, next[1]))
      },

      inner(target, initial)
    },

    find: (fn) => {
      inner: (target) => {
        next: target(),
        if next = null
          null

        if fn(next[1])
          next[1]

        inner(next[0])
      },

      inner(target)
    },

    filter: (fn) => {
      inner: (target) => {
        next: target(),
        () => {
          if next = null
            null

          if fn(next[1])
            [
              inner(next[0]),
              next[1]
            ]

          inner(next[0]).next
        }
      },

      Iterator.new(inner(target))
    }
  }
},

Iterator
