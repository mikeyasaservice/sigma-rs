# Sigma Rule Engine Architecture Diagram

## Current Go Architecture

```mermaid
graph TD
    A[YAML Rules] -->|Parse| B[Rule Parser]
    B --> C[Rule Handle]
    C --> D[Lexer]
    D -->|Tokens| E[Parser]
    E --> F[AST Tree]
    
    G[Events] --> H[Event Interface]
    H --> I[Keyworder]
    H --> J[Selector]
    
    F --> K[Matcher Engine]
    I --> K
    J --> K
    K --> L[Results]
    
    subgraph "Pattern Matching"
        M[String Matchers]
        N[Numeric Matchers]
        O[Regex Engine]
        P[Glob Patterns]
    end
    
    K --> M
    K --> N
    K --> O
    K --> P
```

## Proposed Rust Architecture

```mermaid
graph TD
    A[YAML Rules] -->|serde_yaml| B[Rule Parser]
    B --> C[Compiled Rules]
    C --> D[Lexer Iterator]
    D -->|Token Stream| E[Parser]
    E --> F[AST Tree]
    
    G[Redpanda Events] -->|rdkafka| H[Event Stream]
    H --> I[Event Trait]
    I --> J[Keyworder Trait]
    I --> K[Selector Trait]
    
    F --> L[Matcher Engine]
    J --> L
    K --> L
    L --> M[Results Stream]
    
    subgraph "Optimized Pattern Matching"
        N[SIMD String Match]
        O[Aho-Corasick]
        P[Compiled Regex]
        Q[Optimized Globs]
    end
    
    L --> N
    L --> O
    L --> P
    L --> Q
    
    subgraph "Parallel Processing"
        R[Rayon Thread Pool]
        S[Tokio Runtime]
        T[Event Batching]
    end
    
    H --> T
    T --> R
    M --> S
```

## Component Relationships

```mermaid
classDiagram
    class Event {
        <<trait>>
        +keywords() Vec~String~
        +select(key String) Option~Value~
    }
    
    class Matcher {
        <<trait>>
        +matches(event Event) (bool, bool)
    }
    
    class Branch {
        <<trait>>
        +as_any() Any
    }
    
    class Tree {
        +root Box~Branch~
        +rule Option~RuleHandle~
        +eval(event Event) Option~Result~
    }
    
    class RuleEngine {
        +ruleset Arc~RuleSet~
        +process_batch(events Vec~Event~) Vec~Results~
        +stream_process(source Stream) Stream~Results~
    }
    
    class RedpandaSource {
        +consumer StreamConsumer
        +topic String
        +checkpoint() Result~()~
    }
    
    Event <|-- RedpandaEvent
    Matcher <|-- Branch
    Branch <|-- NodeAnd
    Branch <|-- NodeOr
    Branch <|-- NodeNot
    Tree --> Branch
    RuleEngine --> Tree
    RedpandaSource --> Event
```

## Data Flow

```mermaid
sequenceDiagram
    participant R as Redpanda
    participant S as Event Stream
    participant E as Rule Engine
    participant M as Matcher
    participant P as Pattern Engine
    participant O as Output Stream
    
    R->>S: Consume Events
    S->>E: Event Batch
    E->>E: Deserialize Events
    
    loop For Each Event
        E->>M: Match Against Rules
        M->>P: Apply Patterns
        P->>M: Match Results
        M->>E: Rule Results
    end
    
    E->>O: Stream Results
    O->>R: Publish Results
    E->>R: Checkpoint Offset
```

## Performance Optimization Strategy

```mermaid
graph LR
    A[Raw Events] --> B[Batching]
    B --> C[Parallel Processing]
    C --> D[Pattern Precompilation]
    D --> E[SIMD Acceleration]
    E --> F[Result Aggregation]
    F --> G[Output Stream]
    
    subgraph "Caching Layers"
        H[Rule Cache]
        I[Pattern Cache]
        J[Result Cache]
    end
    
    D --> H
    D --> I
    F --> J
```

## Memory Management

```mermaid
graph TD
    A[Event Stream] --> B[Zero-Copy Deserialize]
    B --> C[Arena Allocator]
    C --> D[AST Nodes]
    
    E[String Patterns] --> F[String Interning]
    F --> G[Pattern Cache]
    
    H[Rule Metadata] --> I[Arc References]
    I --> J[Thread-Safe Access]
    
    subgraph "Lifetime Management"
        K[Event Lifetime 'a]
        L[Rule Lifetime 'static]
        M[Result Lifetime 'b]
    end
    
    B --> K
    D --> L
    G --> M
```