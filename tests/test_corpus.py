import argparse
import copy
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
import urllib.error
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
import corpus
from corpus_transport import IndexCache, valid_path


def encoded(**changes):
    return (json.dumps(dict(name="x", vers="1.0.0", cksum="0"*64, deps=[], features={}, yanked=False) | changes) + "\n").encode()


class Classification(unittest.TestCase):
    def setUp(self):
        self.expected = dict(status="ok", profiles=[dict(name="x", version="1.0.0")], version_union=["x@1.0.0"])
        self.old = {"1/x": encoded()}

    def classify(self, observed=None, replay=None, inputs=None, oracle=True):
        return corpus.classify(self.expected, observed or self.expected, replay or self.expected,
                               self.old, inputs or self.old, oracle)[0]

    def test_unchanged_is_clean(self):
        self.assertEqual(self.classify(), "clean")

    def test_same_inputs_changed_output_is_our_regression(self):
        self.assertEqual(self.classify(observed=dict(self.expected, version_union=[])), "regression")

    def test_upstream_new_version_is_reported(self):
        new = {"1/x": encoded() + encoded(vers="1.1.0")}
        self.assertEqual(self.classify(inputs=new), "upstream")

    def test_backfill_is_reported_but_not_ignored(self):
        self.assertEqual(self.classify(inputs={"1/x": encoded(rust_version="1.70")}), "upstream")

    def test_identity_fields_and_removals_are_never_auto_baselined(self):
        for change in [dict(cksum="1"*64), dict(deps=[dict(name="other")]), dict(features2={"x": []}), dict(links="native")]:
            with self.subTest(change=change):
                self.assertEqual(self.classify(inputs={"1/x": encoded(**change)}), "regression")
        self.assertEqual(self.classify(inputs={"1/x": b""}), "regression")

    def test_changed_upstream_cannot_hide_frozen_replay_regression(self):
        self.assertEqual(self.classify(replay=dict(self.expected, version_union=[]),
                                      inputs={"1/x": encoded() + encoded(vers="1.1.0")}), "regression")

    def test_oracle_mismatch_always_fails_even_with_upstream_changes(self):
        self.assertEqual(self.classify(inputs={"1/x": encoded(rust_version="1.70")}, oracle=False), "conformance_mismatch")

    def test_declared_root_yank_explains_refusal(self):
        self.assertEqual(self.classify(observed=dict(self.expected, status="refused"),
                                      inputs={"1/x": encoded(yanked=True)}), "upstream")

    def test_unrelated_new_version_does_not_excuse_refusal(self):
        self.assertEqual(self.classify(observed=dict(self.expected, status="refused"),
                                      inputs={"1/x": encoded() + encoded(vers="1.1.0")}), "regression")

    def test_missing_reviewed_baseline_fails(self):
        self.assertEqual(corpus.classify(None, self.expected, self.expected, self.old, self.old)[0], "regression")

    def test_protocol_null_normalization_preserves_false_zero_and_empty(self):
        self.assertEqual(corpus.protocol_value(dict(a=None, b=False, c=0, d=[], e={"x": None})),
                         dict(b=False, c=0, d=[], e={}))


class MetadataCache(unittest.TestCase):
    def test_conditional_requests_and_within_run_snapshot(self):
        with tempfile.TemporaryDirectory() as directory:
            cache = IndexCache(directory, "existing-user-agent")
            response = io.BytesIO(encoded())
            response.status = 200
            response.headers = {"ETag": '"v1"', "Last-Modified": "yesterday"}
            cache.opener = Mock()
            cache.opener.open.return_value = response
            self.assertEqual(cache.get("1/x"), encoded())
            self.assertEqual(cache.get("1/x"), encoded())
            self.assertEqual(cache.opener.open.call_count, 1)
            cache2 = IndexCache(directory, "existing-user-agent")
            cache2.opener = Mock()
            cache2.opener.open.side_effect = urllib.error.HTTPError("https://index.crates.io/1/x", 304, "not modified", {}, None)
            self.assertEqual(cache2.get("1/x"), encoded())
            request = cache2.opener.open.call_args.args[0]
            headers = {k.lower():v for k,v in request.header_items()}
            self.assertEqual(headers["if-none-match"], '"v1"')
            self.assertEqual(headers["if-modified-since"], "yesterday")
            self.assertEqual(headers["user-agent"], "existing-user-agent")

    def test_only_canonical_sparse_paths(self):
        for p in ["../x", "/1/x", "1/x?token=secret", "config.json", "archive/x.crate", "aa/bb/%2e"]:
            self.assertFalse(valid_path(p), p)
        self.assertTrue(valid_path("se/rd/serde"))


class CorpusData(unittest.TestCase):
    def test_frozen_roots_and_expected_union_are_intact(self):
        frozen = corpus.corpus(corpus.CORPUS/"frozen-top100.json")
        baseline = corpus.load(corpus.CORPUS/"expected/frozen-top100.json")["expected"]
        self.assertEqual(len(frozen["roots"]), 100)
        self.assertEqual(len(baseline["version_union"]), 186)
        self.assertEqual(len(baseline["roots"]), 100)

    def test_pinned_real_lock_checksums(self):
        for p in corpus.corpus(corpus.CORPUS/"locks.json")["projects"]:
            self.assertEqual(corpus.digest((corpus.CORPUS/p["lock_file"]).read_bytes()), p["lock_sha256"])

    def test_every_case_has_reviewed_baseline_and_explicit_features(self):
        for c in corpus.definitions(False):
            self.assertTrue((corpus.CORPUS/"expected"/(c["id"]+".json")).is_file())
            for r in c["roots"]:
                self.assertIsInstance(r["features"], list)
                self.assertIsInstance(r["default_features"], bool)


class TrackingIssue(unittest.TestCase):
    def test_clean_does_not_create_an_issue(self):
        api = Mock(return_value=[])
        self.assertEqual(corpus.track_report("owner/repo",dict(failed=False,changed=False),"clean",api),"clean")
        self.assertEqual(api.call_count,1)

    def test_change_creates_one_exact_title(self):
        api = Mock(side_effect=[[],{}])
        self.assertEqual(corpus.track_report("owner/repo",dict(failed=False,changed=True),"changed",api),"opened")
        self.assertEqual(api.call_args.args[0],"POST")
        self.assertEqual(api.call_args.args[3]["title"],"Crate corpus report")

    def test_failure_updates_and_reopens_same_issue(self):
        existing=dict(number=3,title=corpus.ISSUE_TITLE,state="closed",body="Human context\n"+corpus.ISSUE_MARKER+"old")
        api=Mock(side_effect=[[existing],{}])
        self.assertEqual(corpus.track_report("owner/repo",dict(failed=True),"failure",api),"updated")
        self.assertEqual(api.call_args.args[2],"issues/3")
        self.assertEqual(api.call_args.args[3]["state"],"open")
        self.assertTrue(api.call_args.args[3]["body"].startswith("Human context"))

    def test_clean_closes_existing_issue_without_spam(self):
        api=Mock(side_effect=[[dict(number=7,title=corpus.ISSUE_TITLE,state="open")],{}])
        self.assertEqual(corpus.track_report("owner/repo",dict(failed=False,changed=False),"clean",api),"closed")
        self.assertEqual(api.call_args.args[3],{"state":"closed"})

    def test_duplicates_refuse_instead_of_creating_another_issue(self):
        api=Mock(return_value=[dict(number=n,title=corpus.ISSUE_TITLE,state="open") for n in [1,2]])
        with self.assertRaisesRegex(ValueError,"multiple"):
            corpus.track_report("owner/repo",dict(failed=True),"bad",api)
        self.assertEqual(api.call_count,1)


class ReleaseGate(unittest.TestCase):
    sha="a"*40

    def test_latest_successful_weekly_is_accepted(self):
        api=Mock(return_value={"workflow_runs":[dict(id=1,status="completed",conclusion="success")]})
        self.assertEqual(corpus.release_check("owner/repo",self.sha,api),1)
        self.assertEqual(api.call_count,1)

    def test_failed_weekly_requires_successful_dispatch_on_exact_head(self):
        api=Mock(side_effect=[{"workflow_runs":[dict(id=1,status="completed",conclusion="failure")]},
            {"workflow_runs":[dict(id=2,head_sha="b"*40,status="completed",conclusion="success"),
                              dict(id=3,head_sha=self.sha,status="completed",conclusion="success")]}])
        self.assertEqual(corpus.release_check("owner/repo",self.sha,api),3)

    def test_missing_failed_or_wrong_head_runs_refuse(self):
        for dispatch in [[],[dict(id=2,head_sha="b"*40,status="completed",conclusion="success")],
                          [dict(id=2,head_sha=self.sha,status="completed",conclusion="failure")]]:
            with self.subTest(dispatch=dispatch):
                api=Mock(side_effect=[{"workflow_runs":[]},{"workflow_runs":dispatch}])
                with self.assertRaisesRegex(ValueError,"corpus gate refused"):
                    corpus.release_check("owner/repo",self.sha,api)

    def test_incomplete_weekly_never_counts_as_a_release_pass(self):
        api=Mock(side_effect=[{"workflow_runs":[dict(id=1,status="in_progress",conclusion=None)]},
                              {"workflow_runs":[]}])
        with self.assertRaisesRegex(ValueError,"corpus gate refused"):
            corpus.release_check("owner/repo",self.sha,api)


class Refresh(unittest.TestCase):
    def test_refresh_preserves_profiles_and_only_proposes_by_default(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "top.json"
            report = Path(directory) / "refresh.json"
            roots = [dict(name=f"crate{i}",version="1.0.0",features=["explicit"],default_features=False) for i in range(100)]
            corpus.dump(path,dict(schema=1,created_at="2026-11-23",source="test",roots=roots))
            original = path.read_bytes()
            response = io.BytesIO(json.dumps(dict(crates=[dict(id=r["name"],max_stable_version="1.1.0") for r in roots])).encode())
            opener = Mock()
            opener.open.return_value = response
            with patch.object(corpus.urllib.request,"build_opener",return_value=opener), patch("sys.stdout",new=io.StringIO()):
                corpus.refresh(argparse.Namespace(file=path,report=report,write=False))
            self.assertEqual(path.read_bytes(),original)
            proposal=corpus.load(report.with_suffix(".proposal.json"))
            self.assertEqual(proposal["roots"][0],dict(roots[0],version="1.1.0"))
            diff=corpus.load(report)
            self.assertEqual(diff["added_roots"],[])
            self.assertEqual(diff["removed_roots"],[])
            self.assertEqual(len(diff["changed_profiles"]),100)
