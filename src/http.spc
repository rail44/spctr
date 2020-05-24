{
  server: {
    state: {
    }
    new: () => {
      inner: (state) => {
        get: (path, handler) => {
          inner({
            ...state,
            state.handlerMap
          })
        },
        serve() => {
        }
      },
      inner(
    }
  }
}
