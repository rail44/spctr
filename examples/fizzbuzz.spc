range: Iterator.range(0, 9000),

fizzbuzz: (i) => {
  is_fizz: i % 3 = 0,
  is_buzz: i % 5 = 0,
  fizz: if is_fizz "fizz" "",
  buzz: if is_buzz "buzz" "",
  
  fizz.concat(buzz)
},

range.map((i) => [i, fizzbuzz(i)]).to_list
