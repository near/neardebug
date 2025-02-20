use bytesize::ByteSize;
use std::fmt::{self, Error, Formatter};

/// For bugs in the runtime itself, crash and die is the usual response.
///
/// See the doc comment on `VMResult` for an explanation what the difference
/// between this and a `FunctionCallError` is.
#[derive(Debug, thiserror::Error)]
pub enum VMRunnerError {
    /// An error that is caused by an operation on an inconsistent state.
    /// E.g. an integer overflow by using a value from the given context.
    #[error("{0}")]
    InconsistentStateError(InconsistentStateError),
}

/// Permitted errors that cause a function call to fail gracefully.
///
/// Occurrence of such errors will be included in the merklize state on chain
/// using a single bit to signal failure vs Success.
///
/// See the doc comment on `VMResult` for an explanation what the difference
/// between this and a `VMRunnerError` is. And see `PartialExecutionStatus`
/// for what gets stored on chain.
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub enum FunctionCallError {
    /// Wasm compilation error
    CompilationError(CompilationError),
    /// Import/export resolve error
    MethodResolveError(MethodResolveError),
    HostError(HostError),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum MethodResolveError {
    MethodEmptyName,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum CompilationError {
    CodeDoesNotExist {
        account_id: Box<str>,
    },
    PrepareError(PrepareError),
    /// This is for defense in depth.
    /// We expect our runtime-independent preparation code to fully catch all invalid wasms,
    /// but, if it ever misses something we’ll emit this error
    WasmerCompileError {
        msg: String,
    },
    /// This is for defense in depth.
    /// We expect our runtime-independent preparation code to fully catch all invalid wasms,
    /// but, if it ever misses something we’ll emit this error
    WasmtimeCompileError {
        msg: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
/// Error that can occur while preparing or executing Wasm smart-contract.
pub enum PrepareError {
    /// Error happened while serializing the module.
    Serialization,
    /// Error happened while deserializing the module.
    Deserialization,
    /// Internal memory declaration has been found in the module.
    InternalMemoryDeclared,
    /// Gas instrumentation failed.
    ///
    /// This most likely indicates the module isn't valid.
    GasInstrumentation,
    /// Stack instrumentation failed.
    ///
    /// This  most likely indicates the module isn't valid.
    StackHeightInstrumentation,
    /// Error happened during instantiation.
    ///
    /// This might indicate that `start` function trapped, or module isn't
    /// instantiable and/or unlinkable.
    Instantiate,
    /// Error creating memory.
    Memory,
    /// Contract contains too many functions.
    TooManyFunctions,
    /// Contract contains too many locals.
    TooManyLocals,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum HostError {
    /// String encoding is bad UTF-16 sequence
    BadUTF16,
    /// String encoding is bad UTF-8 sequence
    BadUTF8,
    /// Exceeded the prepaid gas
    GasExceeded,
    /// Exceeded the maximum amount of gas allowed to burn per contract
    GasLimitExceeded,
    /// Exceeded the account balance
    BalanceExceeded,
    /// Tried to call an empty method name
    EmptyMethodName,
    /// Smart contract panicked
    GuestPanic {
        panic_msg: String,
    },
    /// IntegerOverflow happened during a contract execution
    IntegerOverflow,
    /// `promise_idx` does not correspond to existing promises
    InvalidPromiseIndex {
        promise_idx: u64,
    },
    /// Actions can only be appended to non-joint promise.
    CannotAppendActionToJointPromise,
    /// Returning joint promise is currently prohibited
    CannotReturnJointPromise,
    /// Accessed invalid promise result index
    InvalidPromiseResultIndex {
        result_idx: u64,
    },
    /// Accessed invalid register id
    InvalidRegisterId {
        register_id: u64,
    },
    /// Accessed memory outside the bounds
    MemoryAccessViolation,
    /// VM Logic returned an invalid receipt index
    InvalidReceiptIndex {
        receipt_index: u64,
    },
    /// Iterator index `iterator_index` does not exist
    InvalidIteratorIndex {
        iterator_index: u64,
    },
    /// VM Logic returned an invalid account id
    InvalidAccountId,
    /// VM Logic returned an invalid method name
    InvalidMethodName,
    /// VM Logic provided an invalid public key
    InvalidPublicKey,
    /// `method_name` is not allowed in view calls
    ProhibitedInView {
        method_name: String,
    },
    /// The total number of logs will exceed the limit.
    NumberOfLogsExceeded {
        limit: u64,
    },
    /// The storage key length exceeded the limit.
    KeyLengthExceeded {
        length: u64,
        limit: u64,
    },
    /// The storage value length exceeded the limit.
    ValueLengthExceeded {
        length: u64,
        limit: u64,
    },
    /// The total log length exceeded the limit.
    TotalLogLengthExceeded {
        length: u64,
        limit: u64,
    },
    /// The maximum number of promises within a FunctionCall exceeded the limit.
    NumberPromisesExceeded {
        number_of_promises: u64,
        limit: u64,
    },
    /// The maximum number of input data dependencies exceeded the limit.
    NumberInputDataDependenciesExceeded {
        number_of_input_data_dependencies: u64,
        limit: u64,
    },
    /// The returned value length exceeded the limit.
    ReturnedValueLengthExceeded {
        length: u64,
        limit: u64,
    },
    /// The contract size for DeployContract action exceeded the limit.
    ContractSizeExceeded {
        size: u64,
        limit: u64,
    },
    /// The host function was deprecated.
    Deprecated {
        method_name: String,
    },
    /// General errors for ECDSA recover.
    ECRecoverError {
        msg: String,
    },
    /// Invalid input to alt_bn128 familiy of functions (e.g., point which isn't
    /// on the curve).
    AltBn128InvalidInput {
        msg: String,
    },
    /// Invalid input to ed25519 signature verification function (e.g. signature cannot be
    /// derived from bytes).
    Ed25519VerifyInvalidInput {
        msg: String,
    },
    // Invalid input to bls12381 family of functions
    BLS12381InvalidInput {
        msg: String,
    },
    /// Yield payload length exceeds the maximum permitted.
    YieldPayloadLength {
        length: u64,
        limit: u64,
    },
    /// Yield resumption data id is malformed.
    DataIdMalformed,
    /// Size of the recorded trie storage proof has exceeded the allowed limit.
    RecordedStorageExceeded {
        limit: ByteSize,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub enum VMLogicError {
    /// Errors coming from native Wasm VM.
    HostError(HostError),
    /// An error that is caused by an operation on an inconsistent state.
    InconsistentStateError(InconsistentStateError),
}

impl std::error::Error for VMLogicError {}

/// An error that is caused by an operation on an inconsistent state, such as
/// integer overflow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InconsistentStateError {
    /// Math operation with a value from the state resulted in a integer overflow.
    IntegerOverflow,
}

impl From<HostError> for VMLogicError {
    fn from(err: HostError) -> Self {
        VMLogicError::HostError(err)
    }
}

impl From<InconsistentStateError> for VMLogicError {
    fn from(err: InconsistentStateError) -> Self {
        VMLogicError::InconsistentStateError(err)
    }
}

impl From<PrepareError> for FunctionCallError {
    fn from(err: PrepareError) -> Self {
        FunctionCallError::CompilationError(CompilationError::PrepareError(err))
    }
}

/// Try to convert a general error that happened at the `VMLogic` level to a
/// `FunctionCallError` that can be included in a `VMOutcome`.
///
/// `VMLogicError` have two very different types of errors. Some are just
/// a result from the guest code doing incorrect things, other are bugs in
/// nearcore.
/// The first type can be converted to a `FunctionCallError`, the other becomes
/// a `VMRunnerError` instead and must be treated differently.
impl TryFrom<VMLogicError> for FunctionCallError {
    type Error = VMRunnerError;
    fn try_from(err: VMLogicError) -> Result<Self, Self::Error> {
        match err {
            VMLogicError::HostError(h) => Ok(FunctionCallError::HostError(h)),
            VMLogicError::InconsistentStateError(e) => {
                Err(VMRunnerError::InconsistentStateError(e))
            }
        }
    }
}

impl fmt::Display for VMLogicError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{:?}", self)
    }
}

impl fmt::Display for PrepareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        use PrepareError::*;
        f.write_str(match self {
            Serialization => "Error happened while serializing the module.",
            Deserialization => "Error happened while deserializing the module.",
            InternalMemoryDeclared => "Internal memory declaration has been found in the module.",
            GasInstrumentation => "Gas instrumentation failed.",
            StackHeightInstrumentation => "Stack instrumentation failed.",
            Instantiate => "Error happened during instantiation.",
            Memory => "Error creating memory.",
            TooManyFunctions => "Too many functions in contract.",
            TooManyLocals => "Too many locals declared in the contract.",
        })
    }
}

impl fmt::Display for FunctionCallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        match self {
            FunctionCallError::CompilationError(e) => e.fmt(f),
            FunctionCallError::MethodResolveError(e) => e.fmt(f),
            FunctionCallError::HostError(e) => e.fmt(f),
        }
    }
}

impl fmt::Display for CompilationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        match self {
            CompilationError::CodeDoesNotExist { account_id } => {
                write!(f, "cannot find contract code for account {}", account_id)
            }
            CompilationError::PrepareError(p) => write!(f, "PrepareError: {}", p),
            CompilationError::WasmerCompileError { msg } => {
                write!(f, "Wasmer compilation error: {}", msg)
            }
            CompilationError::WasmtimeCompileError { msg } => {
                write!(f, "Wasmtime compilation error: {}", msg)
            }
        }
    }
}

impl fmt::Display for MethodResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        fmt::Debug::fmt(self, f)
    }
}

impl std::fmt::Display for InconsistentStateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            InconsistentStateError::IntegerOverflow => write!(
                f,
                "Math operation with a value from the state resulted in a integer overflow.",
            ),
        }
    }
}

impl std::fmt::Display for HostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        use HostError::*;
        match self {
            BadUTF8 => write!(f, "String encoding is bad UTF-8 sequence."),
            BadUTF16 => write!(f, "String encoding is bad UTF-16 sequence."),
            GasExceeded => write!(f, "Exceeded the prepaid gas."),
            GasLimitExceeded => {
                write!(
                    f,
                    "Exceeded the maximum amount of gas allowed to burn per contract."
                )
            }
            BalanceExceeded => write!(f, "Exceeded the account balance."),
            EmptyMethodName => write!(f, "Tried to call an empty method name."),
            GuestPanic { panic_msg } => write!(f, "Smart contract panicked: {}", panic_msg),
            IntegerOverflow => write!(f, "Integer overflow."),
            InvalidIteratorIndex { iterator_index } => {
                write!(f, "Iterator index {:?} does not exist", iterator_index)
            }
            InvalidPromiseIndex { promise_idx } => {
                write!(
                    f,
                    "{:?} does not correspond to existing promises",
                    promise_idx
                )
            }
            CannotAppendActionToJointPromise => {
                write!(f, "Actions can only be appended to non-joint promise.")
            }
            CannotReturnJointPromise => {
                write!(f, "Returning joint promise is currently prohibited.")
            }
            InvalidPromiseResultIndex { result_idx } => {
                write!(f, "Accessed invalid promise result index: {:?}", result_idx)
            }
            InvalidRegisterId { register_id } => {
                write!(f, "Accessed invalid register id: {:?}", register_id)
            }
            MemoryAccessViolation => write!(f, "Accessed memory outside the bounds."),
            InvalidReceiptIndex { receipt_index } => {
                write!(
                    f,
                    "VM Logic returned an invalid receipt index: {:?}",
                    receipt_index
                )
            }
            InvalidAccountId => write!(f, "VM Logic returned an invalid account id"),
            InvalidMethodName => write!(f, "VM Logic returned an invalid method name"),
            InvalidPublicKey => write!(f, "VM Logic provided an invalid public key"),
            ProhibitedInView { method_name } => {
                write!(f, "{} is not allowed in view calls", method_name)
            }
            NumberOfLogsExceeded { limit } => {
                write!(f, "The number of logs will exceed the limit {}", limit)
            }
            KeyLengthExceeded { length, limit } => {
                write!(
                    f,
                    "The length of a storage key {} exceeds the limit {}",
                    length, limit
                )
            }
            ValueLengthExceeded { length, limit } => {
                write!(
                    f,
                    "The length of a storage value {} exceeds the limit {}",
                    length, limit
                )
            }
            TotalLogLengthExceeded { length, limit } => {
                write!(
                    f,
                    "The length of a log message {} exceeds the limit {}",
                    length, limit
                )
            }
            NumberPromisesExceeded {
                number_of_promises,
                limit,
            } => write!(
                f,
                "The number of promises within a FunctionCall {} exceeds the limit {}",
                number_of_promises, limit
            ),
            NumberInputDataDependenciesExceeded {
                number_of_input_data_dependencies,
                limit,
            } => {
                write!(
                    f,
                    "The number of input data dependencies {} exceeds the limit {}",
                    number_of_input_data_dependencies, limit
                )
            }
            ReturnedValueLengthExceeded { length, limit } => {
                write!(
                    f,
                    "The length of a returned value {} exceeds the limit {}",
                    length, limit
                )
            }
            ContractSizeExceeded { size, limit } => write!(
                f,
                "The size of a contract code in DeployContract action {} exceeds the limit {}",
                size, limit
            ),
            Deprecated { method_name } => {
                write!(
                    f,
                    "Attempted to call deprecated host function {}",
                    method_name
                )
            }
            AltBn128InvalidInput { msg } => write!(f, "AltBn128 invalid input: {}", msg),
            ECRecoverError { msg } => write!(f, "ECDSA recover error: {}", msg),
            Ed25519VerifyInvalidInput { msg } => {
                write!(f, "ED25519 signature verification error: {}", msg)
            }
            BLS12381InvalidInput { msg } => write!(f, "BLS12-381 invalid input: {}", msg),
            YieldPayloadLength { length, limit } => write!(
                f,
                "Yield resume payload is {length} bytes which exceeds the {limit} byte limit"
            ),
            DataIdMalformed => write!(f, "yield resumption token is malformed"),
            RecordedStorageExceeded { limit } => write!(
                f,
                "Size of the recorded trie storage proof has exceeded the allowed limit ({})",
                limit
            ),
        }
    }
}
