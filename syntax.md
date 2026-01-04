# ADDED

## PRINT

```
// $ is a printing operator.
$2
$2 + 3 // still prints 2
$(2 + 3) // now prints 5

// $ is pretty powerful in that it can be used not only on values, but on other operators too.
2 $+ 3 // equivalent to the previous expression
n $= 2 + 3 // easy way to debug what gets assigned

//- 
in the situation below, $n is both interpreted as an lvalue for the purposes of the assignment,
and as an rvalue with respect to getting printed.
that is to say, if n is already instantiated, its previous value will get printed.
-//
$n = 3 + 4 // prints 5
$new = 2 // prints (undefined variable `new`)
```

## NUMBERS

```
//-
values: 
    `123`     (integers) 
    `123.456` (floating point numbers)
operations: 
    `+`
    `-`
    `*`
    `/`
    `%`
    `^`
`_` can be added to separate digits for legibility
-//
-3 * (1_000 - 2000)
```

## BOOLEANS AND ASK

```
//-
values:
    true
    false
operations: 
    and 
    or
    not
    xor
    ==  
    !=
    >
    <
    >=
    <=
-//
true and false
3 == 3 xor 3 != 3

// ask is an operator that returns the truthiness value of an expression. it behaves like casting a python value to bool
ask 3 // true
ask 3 - 3 // false
ask 0! // full options are true
ask ? // empty options are false
```

## COMMENTS

```
// comment

//-
multiline
comment
-//
```

## VARIABLES

```
// assignment
n = "hello"
n = 2
// note that the assignment evaluates to the assigned value
2 + b = 4 // sets b to 4 and evaluates to 6
```

## EXPRESSIONS AND GROUPINGS

```
//-
in langulo, a program is composed by a sequence of expressions.
an expression is any piece of code that converts to a value.
expressions are separated by newlines.
indenting expressions has no semantic meaning.
-//
1 // --> 1
    2 + 3 // --> 5
    
//-
if you wish for a single expression to span multiple lines, you can use `\` to signal its continuation.
-//
1\
    + 2 +\ 
    3 // --> 6

//- 
to group multiple expressions together into a single expression, called a grouping expression,
you can use `{` `}`. a grouping expression evaluates to its last expression, 
unless it encounters a return expression. in that case, it evaluates to the return value.
-//
{
    "hello"
    "hi"
} // --> "hi"
{
    return "hello"
    "hi"
} // --> "hello"
```

## FUNCTIONS

```
// standard definition
add = |n, m| n + m

//standard usage
five = add(2, 3)

// you can mark a particular argument of a function as `@`
plus = |@, other| @ + other

// a function with an `@` argument can also be called with this infix syntax
five = 2 @ plus 3
five = plus(2, 3) // this syntax is still supported

// the infix syntax makes it easier to chain multiple functions
nine = 2\
    plus(3)\
    plus(4)
    
// if you need a larger function body use a grouping expression
// | @ | {
//  result = @ + 1
//  result * 2
//  return "result is {tmp}"
// }
```

## STRINGS

TODO escape characters, indexing/slicing and making them iterables

```
//- 
values: 
    "string"
    """multiline
    string"""
operations: 
    + (concatenation)
    * (repeat)
-//
"hello" + " world"

// strings can accept placeholders
three = 3
"2
plus
{three} equals
{2+3}"
```

## OPTIONS AND IF/ELSE

```
// an option is an explicit way to indicate that a value could be missing
some_num = 2!
no_num = ?

// if <cond>: <expr> -> gives an empty option on a false conditions. 
// otherwise, evaluates <expr> and wraps it into an option.
if false: 1 // ?, with lazy evaluation of the body
if true: 2 // 2!

// <option> else <expr> -> if the left operand is a full option, evaluates to its unwrapped value.
// if instead it was an empty option, evaluates to whatever <expr> evaluates to.
if false: 
2! else 3 // 2
? else 4 // 4
// note that, streaked together, if <cond>: <expr1> else <expr2> behaves like expected

// some more examples of option usage:
map = |@, fn| if ask @: fn(@ else 0)
map_or = |@, fn, default| @ @map(fn) else default

3! @map(|n| "some({n})") // --> "some(3)"!
?  @map_or(|n| "some({n})", "none") // --> "none"
```

# NOT ADDED AND DESIGN IS NOT SET

## MAPS

```
// a map is a collection of key-value pairs.
map = [0: 1, 1: 2, 2: 3]

// you can also declare a map with multiple shorthands
map = set[1, 2, 3] // [1: true, 2: true, 3: true]
map = list[1, 2, 3] // [0: 1, 1: 2, 2: 3]
map = 1..4 // equivalent to the above

// supported operations:

// indexing: map[key] -> option<value>
map[1] // 2!
map[4] // ?

// assignment: map[key] = value -> updates the map and evaluates to value
map[10] = 4 // map is now [0: 1, 1: 2, 2: 3, 10: 4]

// deletion: del map[key] -> removes the key/value pair from the map (if any) and evaluates to the removed value (if any)
del map[1] // 2! and map is now [0: 1, 2: 3, 10: 4]
del map[4] // ? and map is unchanged

-// 
iter: map iter {
    <body>
}
inside the iter body, you have access to 3 variables: the key, the value, and the index. the iter expression is similar
to a grouping expression in that it evaluates to the last evaluated expression, or the first return expression encountered.
//-

map iter {
    $" #{index} {key}->{value}"
}

// prints "#0 0->1 
// prints #1 2->3
// prints, and evaluates to, #2 10->4"
```