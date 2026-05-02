// middle.spc imports util.spc and re-exports
util: import("./util.spc"),
{
  inc_then_double: (x) => util.double(util.inc(x))
}
