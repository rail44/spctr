l: List.range(0, 1000),

fizzbuzz: (i) => {
  is_fizz: i % 3 = 0,
  is_buzz: i % 5 = 0,
  fizz: if is_fizz "fizz" "",
  buzz: if is_buzz "buzz" "",
  
  fizz.concat(buzz)
},

result: l.to_iter.map(fizzbuzz).to_list,

l.to_iter.map((i) => [i, result[i]]).to_list
