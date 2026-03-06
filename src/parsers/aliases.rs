pub const LOSS_KEYS: &[&str] = &[
    "loss",
    "train_loss",
    "training_loss",
    "train/loss",
    "train.loss",
    "metrics.loss",
    "lm_loss",
    "nll_loss",
];

pub const LEARNING_RATE_KEYS: &[&str] = &[
    "lr",
    "learning_rate",
    "train/learning_rate",
    "train/lr",
    "train.learning_rate",
    "optimizer.lr",
];

pub const STEP_KEYS: &[&str] = &[
    "step",
    "global_step",
    "iteration",
    "train/global_step",
    "train.global_step",
    "_step",
];

pub const THROUGHPUT_KEYS: &[&str] = &["throughput", "tps", "items_per_second", "speed.throughput"];

pub const TOKEN_KEYS: &[&str] = &[
    "tokens",
    "total_tokens",
    "num_tokens",
    "num_input_tokens_seen",
    "train.tokens",
];

pub const EVAL_LOSS_KEYS: &[&str] = &[
    "eval_loss",
    "validation_loss",
    "val/loss",
    "val_loss",
    "eval.loss",
];

pub const GRAD_NORM_KEYS: &[&str] = &[
    "grad_norm",
    "gradient_norm",
    "global_norm",
    "train.grad_norm",
];

pub const SAMPLES_PER_SECOND_KEYS: &[&str] = &[
    "samples_per_second",
    "train_samples_per_second",
    "speed.samples_per_second",
];

pub const STEPS_PER_SECOND_KEYS: &[&str] = &[
    "steps_per_second",
    "train_steps_per_second",
    "speed.steps_per_second",
];

pub const TOKENS_PER_SECOND_KEYS: &[&str] = &[
    "tokens_per_second",
    "train_tokens_per_second",
    "speed.tokens_per_second",
];
