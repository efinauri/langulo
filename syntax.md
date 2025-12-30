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

## BOOLEANS

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

## STATEMENTS AND GROUPINGS

```
//-
in langulo, a program is composed by a sequence of statements.
a statement is any expression that converts to a value.
statements are separated by newlines.
indenting statements has no semantic meaning.
-//
1 // --> 1
    2 + 3 // --> 5
    
//-
if you wish for a single statement to span multiple lines, you can use `\` to signal its continuation.
-//
1\
    + 2 +\ 
    3 // --> 6

//- 
to group multiple statements together into a single statement, called a grouping statement,
you can use `{` `}`. a grouping statement evaluates to its last statement, 
unless it encounters a return statement. in that case, it evaluates to the return value.
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

// postfix definition
plus = |@, other| @ + other

//standard usage
five = add(2, 3)

// postfix usage
five = 2 @ plus 3

// postfix definition makes it easier to chain multiple functions
nine = 2\
    plus(3)\
    plus(4)
    
// if you need a larger function body use a grouping statement
// | @ | {
//  result = @ + 1
//  result * 2
//  return "result is {tmp}"
// }
```

# NOT ADDED BUT DESIGN IS LARGELY SET

## STRINGS

TODO escape characters, indexing/slicing and making them iterables


```
//- 
values: 
    "string"
    "multiline
    string"
operations: 
    + (concatenation)
-//
"hello" + " world"

// strings can accept placeholders
three = 3
"2
plus
{three} equals
{2+3}"
```



# NOT ADDED AND DESIGN IS NOT SET

## OPTIONS AND IF/ELSE

cannot decide how to make definition of an empty option type safe without introducing type annotations everywhere
(and in the first place, do options make sense without static typing)

if/else are solid

```
// an option is an explicit way to indicate that a value could be missing
some_num = 2? // contains a num
no_num = empty


// "if" is a way to define an option relative to a boolean condition
maybe_number = if true 1 // --> 1?
// else unwraps an option safely, giving a fallback value if the option is empty
some_number = maybe_number else 2
```

## ASSOCIATIONS

solid: default value, returning an option of the key

don't like: grouping together list and set, list in particular doesn't sound smart because to keep ordering 
it needs sorteddict 


```
association = [0: "hi", 1: "hello", _: "default"]
association[1] // --> "hi"?
association[999] // --> "default"?
association[1] = "howdy" // --> [0: "hi", 1: "howdy", _: "default"]
association[_] = "new default" // --> [0: "hi", 1: "howdy", _: "new default"]
pop association[1]

//shorthand definitions
list_like = list["hi", "hello"] // [0: "hi", 1: "hello"]
range_like = 3..6 // [0: 3, 1: 4, 2: 5]

set_like = set["hi", "hello"] // ["hi": true, "hello": true]
in = |@, set| set[@] else false
"hi" in set_like // --> true
"howdy" in set_like // --> false
// alternatively, contains = |@, key| @[key] else false
// set_like contains "howdy" --> false
```