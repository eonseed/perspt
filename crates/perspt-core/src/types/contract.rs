use super::*;

/// Test criticality levels for weighted tests
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Criticality {
    /// Critical tests - highest energy penalty on failure
    Critical,
    /// High priority tests
    High,
    /// Low priority tests
    Low,
}

impl Criticality {
    /// Get the energy weight multiplier
    pub fn weight(&self) -> f32 {
        match self {
            Criticality::Critical => 10.0,
            Criticality::High => 3.0,
            Criticality::Low => 1.0,
        }
    }
}

/// Weighted test definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightedTest {
    /// Test name or pattern
    pub test_name: String,
    /// Criticality level
    pub criticality: Criticality,
}

/// Behavioral contract for a node
///
/// Defines the constraints and expectations for an SRBN node.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BehavioralContract {
    /// Required public API signature (hard constraint)
    pub interface_signature: String,
    /// Semantic constraints (e.g., "Use RS256 algorithm")
    pub invariants: Vec<String>,
    /// Anti-patterns to reject (e.g., "no unwrap()")
    pub forbidden_patterns: Vec<String>,
    /// Weighted test cases
    pub weighted_tests: Vec<WeightedTest>,
    /// Energy weights (alpha, beta, gamma) for V(x) calculation
    /// Default: (1.0, 0.5, 2.0) - Logic failures weighted highest
    pub energy_weights: (f32, f32, f32),
}

impl BehavioralContract {
    /// Create a new contract with default weights
    pub fn new() -> Self {
        Self {
            interface_signature: String::new(),
            invariants: Vec::new(),
            forbidden_patterns: Vec::new(),
            weighted_tests: Vec::new(),
            energy_weights: (1.0, 0.5, 2.0), // alpha, beta, gamma from PSP
        }
    }

    /// Get the alpha weight (syntactic energy)
    pub fn alpha(&self) -> f32 {
        self.energy_weights.0
    }

    /// Get the beta weight (structural energy)
    pub fn beta(&self) -> f32 {
        self.energy_weights.1
    }

    /// Get the gamma weight (logic energy)
    pub fn gamma(&self) -> f32 {
        self.energy_weights.2
    }
}

/// Error type for determining retry limits per PSP-4
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ErrorType {
    /// Compilation/syntax/type errors (3 attempts)
    #[default]
    Compilation,
    /// Tool execution failures (5 attempts)
    ToolFailure,
    /// User/reviewer rejection (3 rejections)
    ReviewRejection,
    /// Unknown/other errors (3 attempts default)
    Other,
}

/// Retry policy configuration per PSP-4 specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Max retries for compilation errors (default: 3)
    pub max_compilation_retries: usize,
    /// Max retries for tool failures (default: 5)
    pub max_tool_retries: usize,
    /// Max reviewer rejections before escalation (default: 3)
    pub max_review_rejections: usize,
    /// Current consecutive failures by type
    pub compilation_failures: usize,
    pub tool_failures: usize,
    pub review_rejections: usize,
    /// Last error type encountered
    pub last_error_type: Option<ErrorType>,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            // PSP-4 specified limits
            max_compilation_retries: 3,
            max_tool_retries: 5,
            max_review_rejections: 3,
            compilation_failures: 0,
            tool_failures: 0,
            review_rejections: 0,
            last_error_type: None,
        }
    }
}

impl RetryPolicy {
    /// Record a failure of a specific type
    pub fn record_failure(&mut self, error_type: ErrorType) {
        self.last_error_type = Some(error_type);
        match error_type {
            ErrorType::Compilation => self.compilation_failures += 1,
            ErrorType::ToolFailure => self.tool_failures += 1,
            ErrorType::ReviewRejection => self.review_rejections += 1,
            ErrorType::Other => self.compilation_failures += 1, // Treat as compilation
        }
    }

    /// Check if we should escalate for a specific error type
    pub fn should_escalate(&self, error_type: ErrorType) -> bool {
        match error_type {
            ErrorType::Compilation | ErrorType::Other => {
                self.compilation_failures >= self.max_compilation_retries
            }
            ErrorType::ToolFailure => self.tool_failures >= self.max_tool_retries,
            ErrorType::ReviewRejection => self.review_rejections >= self.max_review_rejections,
        }
    }

    /// Check if any error type has exceeded its limit
    pub fn any_exceeded(&self) -> bool {
        self.compilation_failures >= self.max_compilation_retries
            || self.tool_failures >= self.max_tool_retries
            || self.review_rejections >= self.max_review_rejections
    }

    /// Get remaining attempts for an error type
    pub fn remaining_attempts(&self, error_type: ErrorType) -> usize {
        match error_type {
            ErrorType::Compilation | ErrorType::Other => self
                .max_compilation_retries
                .saturating_sub(self.compilation_failures),
            ErrorType::ToolFailure => self.max_tool_retries.saturating_sub(self.tool_failures),
            ErrorType::ReviewRejection => self
                .max_review_rejections
                .saturating_sub(self.review_rejections),
        }
    }

    /// Get a formatted summary
    pub fn summary(&self) -> String {
        format!(
            "Retries: comp {}/{}, tool {}/{}, review {}/{}",
            self.compilation_failures,
            self.max_compilation_retries,
            self.tool_failures,
            self.max_tool_retries,
            self.review_rejections,
            self.max_review_rejections
        )
    }
}
