# Forge model-attempt generation policy

Scheduled Forge work uses the UUID of its durable `agent_work_item` as the logical generation key. Claim generations
and `storyboard_story_run` rows are dispatch identities: a lease reclaim or automatic requeue may create a new
Story Run, but it keeps the same work item and therefore the same frozen model allowance. Each reservation also
stores its Story Run ID so an operator can trace the attempt to its dispatch.

An explicit fresh generation is a newly authorized work item with a new UUID. Claiming, restarting a worker,
reclaiming a lease, retrying a failed role, or changing `FORGE_MAX_MODEL_TURNS_PER_GENERATION` does not create a
new generation or reset its allowance. The cap is recorded on first use and stays fixed for that generation.

Migration 286 conservatively marks every preexisting work item with a Story Run as `uncertain` and saturates its
allowance. Earlier model attempts cannot be reliably grouped across dispatch runs because the former ledger used
the Story Run ID as its key. An uncertain generation refuses further model launches and requires an operator to
inspect the old run history before choosing a new, explicitly authorized work item. The migration never assumes
that an old work item has a fresh full allowance.

Attempt reservations are permanent once committed. If a process crashes between reservation and provider launch,
the launch result is ambiguous and the reservation remains spent; no reservation is released automatically. A
provider launch that returns an error also remains spent, because the provider may have accepted it before the
connection failed. Only a reservation rejected before insertion consumes no allowance. Corrective attempts use
the same reservation boundary as the first model launch.
