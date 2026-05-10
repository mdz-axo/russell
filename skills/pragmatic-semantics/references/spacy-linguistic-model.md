# spaCy Linguistic Data Model — Complete Reference

## Table of Contents

1. [Part-of-Speech Tagging](#part-of-speech-tagging)
2. [Dependency Parsing](#dependency-parsing)
3. [Named Entity Recognition](#named-entity-recognition)
4. [Morphological Analysis](#morphological-analysis)
5. [Lemmatization](#lemmatization)
6. [Word Vectors and Similarity](#word-vectors-and-similarity)
7. [Pattern Matching Language](#pattern-matching-language)

---

## Part-of-Speech Tagging

spaCy uses two tag sets: coarse-grained Universal Dependencies (UD) tags and fine-grained language-specific tags.[^ud]

[^ud]: Universal Dependencies: [universaldependencies.org](https://universaldependencies.org/). De Marneffe, M.-C. et al. (2021). *Universal Dependencies*. Computational Linguistics, 47(2).

### Coarse POS Tags (Universal Dependencies)

Accessed via `token.pos_`:

| Tag | Description | Examples |
|-----|-------------|---------|
| `ADJ` | Adjective | big, old, green |
| `ADP` | Adposition (preposition) | in, to, during |
| `ADV` | Adverb | very, tomorrow, down |
| `AUX` | Auxiliary verb | is, has, will, should |
| `CCONJ` | Coordinating conjunction | and, or, but |
| `DET` | Determiner | a, the, this, every |
| `INTJ` | Interjection | hello, oh, wow |
| `NOUN` | Noun | girl, cat, tree |
| `NUM` | Numeral | 42, three, 3.14 |
| `PART` | Particle | not, 's, up (phrasal verb) |
| `PRON` | Pronoun | I, you, he, herself |
| `PROPN` | Proper noun | Mary, London, IBM |
| `PUNCT` | Punctuation | ., (, ), - |
| `SCONJ` | Subordinating conjunction | if, while, that |
| `SYM` | Symbol | $, %, +, = |
| `VERB` | Verb | run, eat, is |
| `X` | Other | foreign words, typos |
| `SPACE` | Whitespace | (space, newline) |

### Fine-Grained Tags (Penn Treebank — English)

Accessed via `token.tag_`:

| Tag | Description | Example |
|-----|-------------|---------|
| `NN` | Noun, singular or mass | dog |
| `NNS` | Noun, plural | dogs |
| `NNP` | Proper noun, singular | London |
| `NNPS` | Proper noun, plural | Americans |
| `VB` | Verb, base form | be |
| `VBD` | Verb, past tense | was |
| `VBG` | Verb, gerund/present participle | being |
| `VBN` | Verb, past participle | been |
| `VBP` | Verb, non-3rd person singular present | am |
| `VBZ` | Verb, 3rd person singular present | is |
| `JJ` | Adjective | big |
| `JJR` | Adjective, comparative | bigger |
| `JJS` | Adjective, superlative | biggest |
| `RB` | Adverb | quickly |
| `RBR` | Adverb, comparative | faster |
| `RBS` | Adverb, superlative | fastest |
| `PRP` | Personal pronoun | I, you, he |
| `PRP$` | Possessive pronoun | my, your, his |
| `DT` | Determiner | the, a, this |
| `IN` | Preposition/subordinating conjunction | in, of, if |
| `CC` | Coordinating conjunction | and, or, but |
| `MD` | Modal | can, will, should |
| `TO` | "to" | to |
| `CD` | Cardinal number | 42, three |

> **Source**: Marcus, M. et al. (1993). *Building a Large Annotated Corpus of English: The Penn Treebank*. Computational Linguistics, 19(2). [ACM DL](https://aclanthology.org/J93-2004/)

---

## Dependency Parsing

spaCy uses a **transition-based** dependency parser (arc-eager algorithm by default) that assigns a syntactic head and dependency relation to each token.[^parser]

[^parser]: Honnibal, M. & Johnson, M. (2015). *An Improved Non-Monotonic Transition System for Dependency Parsing*. EMNLP. [ACL Anthology](https://aclanthology.org/D15-1162/)

### Dependency Visualization

```python
from spacy import displacy

doc = nlp("The quick brown fox jumped over the lazy dog")
displacy.render(doc, style="dep", jupyter=True)
# Or save to HTML:
html = displacy.render(doc, style="dep")
```

### Core Dependency Relations (Universal Dependencies)

| Relation | Description | Example (head ← dependent) |
|----------|-------------|----------------------------|
| `nsubj` | Nominal subject | fox ← jumped ("fox jumped") |
| `dobj` / `obj` | Direct object | ate ← cake ("ate cake") |
| `iobj` | Indirect object | gave ← me ("gave me") |
| `amod` | Adjectival modifier | fox ← quick ("quick fox") |
| `advmod` | Adverbial modifier | ran ← quickly ("ran quickly") |
| `det` | Determiner | fox ← the ("the fox") |
| `prep` / `case` | Prepositional marker | over ← jumped ("jumped over") |
| `pobj` / `obl` | Object of preposition | dog ← over ("over the dog") |
| `aux` | Auxiliary | running ← is ("is running") |
| `neg` | Negation modifier | running ← not ("not running") |
| `compound` | Compound word | York ← New ("New York") |
| `conj` | Conjunct | cats ← dogs ("cats and dogs") |
| `cc` | Coordinating conjunction | and ← cats ("cats and") |
| `relcl` | Relative clause | man ← saw ("man who I saw") |
| `advcl` | Adverbial clause | left ← finished ("left when finished") |
| `xcomp` | Open clausal complement | wants ← go ("wants to go") |
| `ccomp` | Clausal complement | said ← left ("said that he left") |
| `ROOT` | Root of sentence | (head of tree) |
| `punct` | Punctuation | . ← ROOT |

### Dependency Tree Traversal

```python
doc = nlp("The quick brown fox jumped over the lazy dog")

# Navigate the tree
for token in doc:
    print(f"{token.text:12} {token.dep_:10} {token.head.text:12} [{', '.join([c.text for c in token.children])}]")

# Find subject and object
root = [t for t in doc if t.dep_ == "ROOT"][0]
subjects = [t for t in root.lefts if t.dep_ in ("nsubj", "nsubjpass")]
objects = [t for t in root.rights if t.dep_ in ("dobj", "obj")]

# Get subtree (all descendants)
for token in doc:
    subtree = list(token.subtree)
    print(f"{token.text} subtree: {[t.text for t in subtree]}")
```

### Parser Algorithms

| Algorithm | Type | Speed | Accuracy | spaCy Default |
|-----------|------|-------|----------|---------------|
| **Arc-eager** | Transition-based | Fastest | Good | Yes (non-trf models) |
| **Arc-standard** | Transition-based | Fast | Good | No |
| **Biaffine** | Graph-based | Medium | Best | Yes (trf models) |

---

## Named Entity Recognition

### Entity Types (OntoNotes 5)

| Label | Description | Examples |
|-------|-------------|---------|
| `PERSON` | People, including fictional | Albert Einstein, Sherlock Holmes |
| `NORP` | Nationalities, religious/political groups | American, Buddhist, Republican |
| `FAC` | Buildings, airports, highways | the White House, I-95 |
| `ORG` | Companies, agencies, institutions | Google, UN, MIT |
| `GPE` | Countries, cities, states | France, London, California |
| `LOC` | Non-GPE locations | Asia, the Nile, Mount Everest |
| `PRODUCT` | Vehicles, weapons, foods (not services) | iPhone, Boeing 747 |
| `EVENT` | Named hurricanes, wars, sports events | World War II, Super Bowl |
| `WORK_OF_ART` | Titles of books, songs, etc. | The Great Gatsby |
| `LAW` | Named legal documents | the First Amendment |
| `LANGUAGE` | Named languages | English, Mandarin |
| `DATE` | Absolute or relative dates | June 2024, yesterday, last week |
| `TIME` | Times of day | 3:00 PM, morning |
| `PERCENT` | Percentages | 25%, thirty percent |
| `MONEY` | Monetary values | $10, fifty dollars |
| `QUANTITY` | Measurements | 100 km, three pounds |
| `ORDINAL` | Ordinal numbers | first, 2nd |
| `CARDINAL` | Cardinal numbers not covered by others | 42, three |

### Entity Encoding Schemes

**IOB (Inside-Outside-Beginning)**:

```
The    O
quick  O
brown  O
fox    O
Apple  B-ORG
Inc    I-ORG
.      O
```

| Tag | Meaning |
|-----|---------|
| `B-LABEL` | Beginning of entity |
| `I-LABEL` | Inside (continuation) of entity |
| `O` | Outside any entity |

**BILUO (Begin-Inside-Last-Unit-Outside)** — more precise:

| Tag | Meaning |
|-----|---------|
| `B-LABEL` | Beginning of multi-token entity |
| `I-LABEL` | Inside multi-token entity |
| `L-LABEL` | Last token of multi-token entity |
| `U-LABEL` | Unit (single-token entity) |
| `O` | Outside any entity |

```python
# Access entity IOB tags
for token in doc:
    print(token.text, token.ent_iob_, token.ent_type_)

# Convert between schemes
from spacy.training import offsets_to_biluo_tags, biluo_tags_to_offsets
biluo = offsets_to_biluo_tags(doc, entities)
```

### Entity Visualization

```python
from spacy import displacy

doc = nlp("Apple is looking at buying UK startup for $1 billion")
displacy.render(doc, style="ent", jupyter=True)
# Colors entities inline with labels
```

> **Source**: Sang, E.F.T.K. & De Meulder, F. (2003). *Introduction to the CoNLL-2003 Shared Task: Language-Independent Named Entity Recognition*. CoNLL. [ACL Anthology](https://aclanthology.org/W03-0419/)

---

## Morphological Analysis

Accessed via `token.morph`:

```python
doc = nlp("She was running quickly")
for token in doc:
    print(token.text, token.morph)
# She      Case=Nom|Gender=Fem|Number=Sing|Person=3|PronType=Prs
# was      Mood=Ind|Number=Sing|Person=3|Tense=Past|VerbForm=Fin
# running  Aspect=Prog|Tense=Pres|VerbForm=Part
# quickly  Degree=Pos
```

### Key Morphological Features (Universal Dependencies)

| Feature | Values | Example |
|---------|--------|---------|
| `Number` | Sing, Plur | cat/cats |
| `Person` | 1, 2, 3 | I/you/she |
| `Tense` | Past, Pres, Fut | ran/runs/will run |
| `Aspect` | Perf, Prog | has run / is running |
| `Mood` | Ind, Imp, Sub | runs / run! / if he run |
| `Voice` | Act, Pass | writes / is written |
| `Case` | Nom, Acc, Gen, Dat | she / her / her / her |
| `Gender` | Masc, Fem, Neut | he / she / it |
| `Degree` | Pos, Cmp, Sup | big / bigger / biggest |
| `VerbForm` | Fin, Inf, Part, Ger | runs / run / running / running |

---

## Lemmatization

spaCy offers three lemmatization strategies:

| Strategy | Method | Speed | Accuracy | When to Use |
|----------|--------|-------|----------|-------------|
| **Lookup** | Dictionary mapping | Fast | Medium | Default for most languages |
| **Rule-based** | POS + rules | Fast | Good | English and morphologically regular languages |
| **Neural** | Edit tree classifier | Medium | Best | When POS tagger is available |

```python
doc = nlp("The mice were running around the houses")
for token in doc:
    print(token.text, "->", token.lemma_)
# mice -> mouse
# were -> be
# running -> run
# houses -> house
```

---

## Word Vectors and Similarity

### Static Vectors (GloVe/fastText)

Available in `_md` and `_lg` models:

```python
nlp = spacy.load("en_core_web_lg")  # 685K 300d GloVe vectors

# Token vectors
token = nlp("apple")[0]
print(token.vector.shape)  # (300,)
print(token.has_vector)    # True

# Document vectors (average of token vectors)
doc1 = nlp("I like cats")
doc2 = nlp("I like dogs")
print(doc1.similarity(doc2))  # ~0.92

# Find similar words
from spacy.vocab import Vocab
queries = ["king", "queen", "man", "woman"]
for word in queries:
    lexeme = nlp.vocab[word]
    print(word, lexeme.vector_norm)
```

### Contextual Embeddings (Transformers)

Available in `_trf` models:

```python
nlp = spacy.load("en_core_web_trf")
doc = nlp("The bank by the river")

# Transformer output is in doc._.trf_data
# Each token gets a contextual vector
# "bank" near "river" has different embedding than "bank" near "money"
```

### Vector Operations

```python
# Analogy: king - man + woman = ?
from numpy import dot
from numpy.linalg import norm

def cosine_sim(a, b):
    return dot(a, b) / (norm(a) * norm(b))

king = nlp.vocab["king"].vector
man = nlp.vocab["man"].vector
woman = nlp.vocab["woman"].vector
result = king - man + woman
# Find closest word to result vector

# Most similar words
ms = nlp.vocab.vectors.most_similar(result.reshape(1, -1), n=10)
```

> **Source**: Pennington, J. et al. (2014). *GloVe: Global Vectors for Word Representation*. EMNLP. [nlp.stanford.edu/projects/glove](https://nlp.stanford.edu/projects/glove/). Mikolov, T. et al. (2013). *Efficient Estimation of Word Representations in Vector Space*. [arXiv:1301.3781](https://arxiv.org/abs/1301.3781).

---

## Pattern Matching Language

### Token Pattern Attributes

The Matcher pattern language matches on token-level attributes:

| Attribute | Type | Description | Example |
|-----------|------|-------------|---------|
| `TEXT` | str | Exact text | `{"TEXT": "Apple"}` |
| `LOWER` | str | Lowercase text | `{"LOWER": "apple"}` |
| `POS` | str | Coarse POS tag | `{"POS": "NOUN"}` |
| `TAG` | str | Fine-grained POS | `{"TAG": "NNP"}` |
| `DEP` | str | Dependency relation | `{"DEP": "nsubj"}` |
| `LEMMA` | str | Lemma | `{"LEMMA": "be"}` |
| `SHAPE` | str | Word shape | `{"SHAPE": "Xxxxx"}` |
| `ENT_TYPE` | str | Entity type | `{"ENT_TYPE": "ORG"}` |
| `IS_ALPHA` | bool | All letters | `{"IS_ALPHA": True}` |
| `IS_DIGIT` | bool | All digits | `{"IS_DIGIT": True}` |
| `IS_UPPER` | bool | All uppercase | `{"IS_UPPER": True}` |
| `IS_LOWER` | bool | All lowercase | `{"IS_LOWER": True}` |
| `IS_TITLE` | bool | Titlecase | `{"IS_TITLE": True}` |
| `IS_PUNCT` | bool | Punctuation | `{"IS_PUNCT": True}` |
| `IS_STOP` | bool | Stop word | `{"IS_STOP": False}` |
| `LENGTH` | int | Token length | `{"LENGTH": {">=": 3}}` |
| `MORPH` | str | Morphological features | `{"MORPH": {"IS_SUPERSET": ["Number=Plur"]}}` |

### Quantifiers

| Operator | Description | Regex Equivalent |
|----------|-------------|-----------------|
| `!` | Negation (match 0 times) | `(?!...)` |
| `?` | Optional (0 or 1) | `?` |
| `+` | One or more | `+` |
| `*` | Zero or more | `*` |

### Complex Pattern Examples

```python
# Noun phrase: determiner? + adjective* + noun+
pattern_np = [
    {"POS": "DET", "OP": "?"},
    {"POS": "ADJ", "OP": "*"},
    {"POS": "NOUN", "OP": "+"}
]

# Email-like pattern
pattern_email = [
    {"LIKE_EMAIL": True}
]

# Version number (e.g., "v3.5.1")
pattern_version = [
    {"TEXT": {"REGEX": r"v?\d+\.\d+(\.\d+)?"}},
]

# Multi-word entity with flexible middle
pattern_compound = [
    {"LOWER": "united"},
    {"IS_ALPHA": True, "OP": "?"},
    {"LOWER": "america"}
]

# Using IN for alternatives
pattern_modal = [
    {"LEMMA": {"IN": ["can", "could", "may", "might", "should", "must"]}},
    {"POS": "VERB"}
]
```

> **Source**: spaCy docs — [Rule-based Matching](https://spacy.io/usage/rule-based-matching). [Matcher API](https://spacy.io/api/matcher).

---

## References

### Academic
- Marcus, M. et al. (1993). *Building a Large Annotated Corpus of English: The Penn Treebank*. Computational Linguistics.
- De Marneffe, M.-C. et al. (2021). *Universal Dependencies*. Computational Linguistics, 47(2).
- Honnibal, M. & Johnson, M. (2015). *An Improved Non-Monotonic Transition System for Dependency Parsing*. EMNLP.
- Pennington, J. et al. (2014). *GloVe: Global Vectors for Word Representation*. EMNLP.
- Sang, E.F.T.K. & De Meulder, F. (2003). *CoNLL-2003 Named Entity Recognition*. CoNLL.

### Documentation
- spaCy API reference: [spacy.io/api](https://spacy.io/api)
- Universal Dependencies: [universaldependencies.org](https://universaldependencies.org/)
- Penn Treebank tagset: [repository.upenn.edu/cis_reports/570](https://repository.upenn.edu/cis_reports/570/)
- OntoNotes 5 entity types: [catalog.ldc.upenn.edu](https://catalog.ldc.upenn.edu/LDC2013T19)
