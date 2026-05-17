// FizzBuzz over [0, 100). Returns a list of strings — number-typed slots
// would force a heterogeneous list, so `i` is interpolated as a string.
fizzbuzz: (i) =>
  if i % 15 == 0 then "FizzBuzz"
  else if i % 3 == 0 then "Fizz"
  else if i % 5 == 0 then "Buzz"
  else "${i}",

List.map(List.range(0, 100), fizzbuzz)
