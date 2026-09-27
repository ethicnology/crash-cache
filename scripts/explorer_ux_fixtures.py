"""Small synthetic datasets exercising empty, incomplete and busy dashboard states."""
from __future__ import annotations

import datetime as dt
import hashlib
import uuid

from benchmark_sentry import timestamp
from seed_observability_demo import encode_envelope

NAMESPACE = uuid.UUID('d3b6ebcc-ec1f-4c87-b71e-87a791334884')
SYSTEMS = (('java', 'Android'), ('native', 'Linux'), ('cocoa', 'macOS'))


def generate(scenario: str, date: dt.date) -> tuple[list[bytes], dict]:
    """Return deterministic envelopes; no HTTP or filesystem effects.

    partial: three session starts, only one platform correlation, two active
    identities plus one anonymous observation, and errors with/without stacks.
    volume: 350 errors representing 300 fingerprints and 80 installations.
    empty: no envelopes. All dates are UTC; no real user data is generated.
    """
    if scenario not in ('empty', 'partial', 'volume'):
        raise ValueError('scenario must be empty, partial or volume')
    namespace = uuid.uuid5(NAMESPACE, f'{scenario}:{date}')
    base = dt.datetime.combine(date, dt.time(12), dt.timezone.utc)

    def identity(label: str) -> str:
        return str(uuid.uuid5(namespace, label))

    def event(index: int, kind: str, person: int | None) -> dict:
        platform, system = SYSTEMS[index % len(SYSTEMS)]
        value = {'event_id': identity(f'event:{index}').replace('-', ''),
                 'timestamp': timestamp(base + dt.timedelta(seconds=index)),
                 'platform': platform, 'environment': 'synthetic-ux',
                 'release': 'synthetic-ux@1.0.0', 'level': 'info',
                 'tags': {'synthetic': 'true', 'dataset': f'ux-{scenario}', 'event_kind': kind},
                 'contexts': {'os': {'name': system, 'version': 'demo'},
                              'device': {'model': f'Synthetic {system} device'}}}
        if person is not None:
            value['user'] = {'id': identity(f'installation:{person}')}
        return value

    def error(index: int, issue: int, person: int, stack: bool) -> dict:
        value = event(index, 'error', person)
        value.update(level='error', message=f'Synthetic operation {issue} failed',
                     fingerprint=['synthetic-ux', scenario, f'operation-{issue}'])
        exception = {'type': 'SyntheticOperationError', 'value': value['message']}
        if stack:
            exception['stacktrace'] = {'frames': [
                {'filename': 'synthetic/workflow.rs', 'function': 'run_operation',
                 'lineno': 20, 'in_app': True},
                {'filename': 'synthetic/operation.rs', 'function': f'operation_{issue}',
                 'lineno': 42, 'in_app': True,
                 'context_line': 'return execute(request);', 'vars': {'attempt': '1'}}]}
        value['exception'] = {'values': [exception]}
        return value

    envelopes: list[bytes] = []
    expected = {'events': 0, 'sessions': 0, 'errors': 0, 'distinct_fingerprints': 0}
    if scenario == 'partial':
        for index in range(3):
            session_id = identity(f'session:{index}')
            activity = event(index, 'app_activity', index if index != 1 else None)
            if index != 2:
                activity['tags']['app_session_id'] = session_id
            session = {'sid': session_id, 'did': identity(f'installation:{index}'),
                       'seq': 0, 'init': True, 'started': timestamp(base),
                       'timestamp': timestamp(base), 'status': 'ok', 'errors': 0,
                       'attrs': {'release': 'synthetic-ux@1.0.0', 'environment': 'synthetic-ux'}}
            envelopes.append(encode_envelope([({'type': 'event'}, activity),
                                              ({'type': 'session'}, session)], base))
        for index in range(2):
            envelopes.append(encode_envelope([({'type': 'event'}, error(10 + index, index, index, index == 0))], base))
        expected.update(events=5, sessions=3, errors=2, distinct_fingerprints=2,
                        activity_without_identity=1, sessions_without_platform_match=2,
                        errors_without_stack=1, active_installations=2)
    elif scenario == 'volume':
        for index in range(350):
            # Repeated faults affect a different installation than their first
            # occurrence, making the impact distribution visible in issue lists.
            envelopes.append(encode_envelope([({'type': 'event'}, error(index, index % 300, index % 80, True))], base))
        expected.update(events=350, errors=350, distinct_fingerprints=300,
                        error_affected_installations=80)
    return envelopes, {'scenario': scenario, 'synthetic': True, 'date': date.isoformat(),
                       'expected': expected, 'envelopes': len(envelopes),
                       'payload_bytes': sum(map(len, envelopes)),
                       'sha256': hashlib.sha256(b''.join(envelopes)).hexdigest()}
