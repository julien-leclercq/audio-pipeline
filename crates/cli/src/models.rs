use uuid::Uuid;

struct Pipeline {
    id: Uuid,
    name: String,
}

enum StepStatus {
    Queued,
    Waiting,
    Processing,
    Done,
    Error,
}

struct Step<State>
where
    State: Deserialize + Serialize,
{
    id: Uuid,
    pipeline_id: Uuid,
    name: String,
    status: StepStatus,
    state: State,
}
