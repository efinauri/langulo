# ADDED

## PRINT

```
// <$expr> prints the expr right after it to stdout and evaluates to expr
$2
$2 + 3 // still prints 2
$(2 + 3) // now prints 5
n = $(2 + 3) // prints 5 and sets n to 5. 
// in practice, to quickly print an assignment you would usually write
// $n = 2 + 3
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

# NOT ADDED BUT DESIGN IS LARGELY SET

## VARIABLES

TODO compount assignments aren't implemented yet

```
// assignment
n = "hello"
n = 2
// note that the assignment evaluates to the assigned value
2 + b = 4 // sets b to 4 and evaluates to 6

// compound assignments: +=, *=, -=, /=, %=, ^=
x = 1
y += n * 3
```

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

## FUNCTIONS

Things that are undecided are all related to the `STATEMENTS AND GROUPINGS` section

```
// standard definition
add = |n, m| n + m

//standard usage
five = add(2, 3)

// postfix definition
plus = |@, other| @ + other

// postfix usage
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

# NOT ADDED AND DESIGN IS NOT SET

## STATEMENTS AND GROUPINGS

multiline statements are maybe ugly with `\`

```
//-
a statement is any expression that converts to a value.
statements are separated by newlines.
indenting statements has no semantic meaning.
-//
1 // --> 1
    2 + 3 // --> 5
    
//-
statements by default span a single line.
if you wish for a statement to span multiple lines, you can use `\`
-//
1\
    + 2 +\ 
    3 // --> 6

//- 
to group multiple statements together into a single statement you can use `{` `}`.
a grouping statement evaluates to its last statement, unless it encounters a return.
in that case, it evaluates to the return value.
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