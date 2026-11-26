import argparse
import copy
import io
import json
from pathlib import Path
import sys
import subprocess
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
        self.assertEqual(self.classify(inputs={"1/x": encoded(rust_version="1.70")}, oracle=False), "regression")

    def test_declared_root_yank_explains_refusal(self):
        self.assertEqual(self.classify(observed=dict(self.expected, status="refused",version_union=[]),
                                      inputs={"1/x": encoded(yanked=True)}), "upstream_needs_review")

    def test_unrelated_new_version_does_not_excuse_refusal(self):
        self.assertEqual(self.classify(observed=dict(self.expected, status="refused"),
                                      inputs={"1/x": encoded() + encoded(vers="1.1.0")}), "regression")

    def test_unrelated_crate_update_cannot_excuse_changed_selection(self):
        self.old["1/y"] = encoded(name="y")
        new = dict(self.old, **{"1/y": encoded(name="y") + encoded(name="y", vers="1.1.0")})
        self.assertEqual(self.classify(observed=dict(self.expected, version_union=[]), inputs=new), "regression")

    def test_formatting_only_change_cannot_excuse_changed_selection(self):
        formatted = (json.dumps(json.loads(encoded()), separators=(",", ":")) + "\n").encode()
        self.assertEqual(self.classify(observed=dict(self.expected, version_union=[]), inputs={"1/x": formatted}), "regression")

    def test_real_new_version_explains_same_crate_selection_change(self):
        observed = dict(self.expected, version_union=["x@1.1.0"])
        self.assertEqual(self.classify(observed=observed, inputs={"1/x": encoded() + encoded(vers="1.1.0")}), "upstream_needs_review")

    def test_overselection_growth_is_a_regression_despite_upstream_updates(self):
        new = {"1/x": encoded() + encoded(vers="1.1.0")}
        before=[dict(profile={"name":"x"}, native_only=[])]
        after=[dict(profile={"name":"x"}, native_only=["x@1.0.0"])]
        self.assertEqual(corpus.classify(self.expected, dict(self.expected, version_union=["x@1.0.0","x@1.1.0"]),
            self.expected,self.old,new,True,before,after)[0],"regression")

    def test_real_yank_explains_selection_change(self):
        observed=dict(self.expected,version_union=[])
        self.assertEqual(self.classify(observed=observed,inputs={"1/x":encoded(yanked=True)}),"upstream_needs_review")

    def test_backfill_cannot_excuse_resolver_state_change(self):
        self.expected["version_details"]={"x@1.0.0":{"features":[]}}
        changed=dict(self.expected,version_details={"x@1.0.0":{"features":["invented"]}})
        self.assertEqual(self.classify(observed=changed,inputs={"1/x":encoded(rust_version="1.70")}),"regression")

    def test_unchanged_reviewed_overselection_is_allowed_but_new_extras_fail(self):
        baseline=[dict(profile={"name":"x"},native_only=["x@1.0.0"])]
        self.assertEqual(corpus.over_selection_growth(baseline,baseline),[])
        changed=[dict(profile={"name":"x"},native_only=["x@1.1.0"])]
        self.assertTrue(corpus.over_selection_growth(baseline,changed))
        self.assertTrue(corpus.over_selection_growth(None,changed))

    def test_actual_backfilled_provenance_is_reported(self):
        before,after=encoded(),encoded(rust_version="1.70")
        self.expected.update(selected_rows={"x@1.0.0":json.loads(before)},index_line_sha256={"x@1.0.0":corpus.digest(before)})
        observed=dict(self.expected,selected_rows={"x@1.0.0":json.loads(after)},index_line_sha256={"x@1.0.0":corpus.digest(after)})
        self.assertEqual(self.classify(observed=observed,inputs={"1/x":after}),"upstream")
        observed["index_line_sha256"]={"x@1.0.0":"invented"}
        self.assertEqual(self.classify(observed=observed,inputs={"1/x":after}),"regression")

    def test_reason_only_changes_are_informational_with_unchanged_inputs(self):
        self.expected["version_details"] = {"x@1.0.0": dict(reasons=["old"], primary_reason="old", sources=["a"], source_ordinal=1, root_aliases=["a"])}
        observed = copy.deepcopy(self.expected)
        observed["version_details"]["x@1.0.0"].update(reasons=["new"], primary_reason="new", sources=["b"], source_ordinal=2, root_aliases=["b"])
        label, details = corpus.classify(self.expected, observed, self.expected, self.old, self.old)
        self.assertEqual(label, "clean")
        self.assertTrue(any(d.get("kind") == "informational" for d in details))

    def test_reason_only_frozen_replay_drift_is_informational(self):
        replay = dict(self.expected, version_details={"x@1.0.0": {"reasons": ["new explanation"]}})
        label, details = corpus.classify(self.expected, replay, replay, self.old, self.old)
        self.assertEqual(label, "clean")
        self.assertEqual({d["phase"] for d in details}, {"live", "frozen_replay"})

    def test_reason_drift_cannot_hide_selection_checksum_or_served_bytes_changes(self):
        for mutation in [dict(version_union=[]),
                         dict(version_details={"x@1.0.0": {"cksum": "1"*64, "reasons": ["new"]}}),
                         dict(index_line_sha256={"x@1.0.0": "invented"})]:
            with self.subTest(mutation=mutation):
                self.assertEqual(self.classify(observed=dict(self.expected, **mutation)), "regression")

    def test_reason_drift_cannot_hide_forged_provenance_during_real_upstream_change(self):
        before = encoded()
        self.expected.update(selected_rows={"x@1.0.0": json.loads(before)},
                             index_line_sha256={"x@1.0.0": corpus.digest(before)})
        inputs = {"1/x": before + encoded(vers="1.1.0")}
        for field in ("cksum", "index_line_sha256"):
            observed = copy.deepcopy(self.expected)
            observed["version_details"] = {"x@1.0.0": {"reasons": ["new explanation"]}}
            if field == "cksum":
                observed["selected_rows"]["x@1.0.0"]["cksum"] = "1"*64
            else:
                observed["index_line_sha256"]["x@1.0.0"] = "invented"
            with self.subTest(field=field):
                self.assertEqual(self.classify(observed=observed,inputs=inputs), "regression")

    def test_inconsistent_checksum_or_served_version_is_not_excused_by_cargo_and_new_index_version(self):
        old = encoded()
        self.expected.update(selected_rows={"x@1.0.0":json.loads(old)},
            index_line_sha256={"x@1.0.0":corpus.digest(old)},
            version_details={"x@1.0.0":dict(name="x",version="1.0.0",cksum="0"*64)})
        new = encoded(vers="1.1.0")
        proof = [dict(profile=self.expected["profiles"][0],native_only=[],
                      cargo_versions=["x@1.0.0"],filtered_versions=["x@1.0.0"],ok=True)]
        for field in ("checksum", "served_version"):
            observed = copy.deepcopy(self.expected)
            observed["version_details"]["x@1.0.0"]["reasons"] = ["new explanation"]
            if field == "checksum":
                observed["version_details"]["x@1.0.0"]["cksum"] = "1"*64
            else:
                observed["selected_rows"]["x@1.0.0"] = json.loads(new)
                observed["index_line_sha256"]["x@1.0.0"] = corpus.digest(new)
            with self.subTest(field=field):
                self.assertEqual(corpus.classify(self.expected,observed,self.expected,self.old,
                    {"1/x":old+new},True,proof,proof)[0], "regression")

    def test_reason_drift_cannot_hide_cargo_disagreement(self):
        observed = dict(self.expected, version_details={"x@1.0.0": {"reasons": ["new"]}})
        self.assertEqual(self.classify(observed=observed, oracle=False), "regression")

    def test_missing_reviewed_baseline_fails(self):
        self.assertEqual(corpus.classify(None, self.expected, self.expected, self.old, self.old)[0], "regression")

    def test_protocol_null_normalization_preserves_false_zero_and_empty(self):
        self.assertEqual(corpus.protocol_value(dict(a=None, b=False, c=0, d=[], e={"x": None})),
                         dict(b=False, c=0, d=[], e={}))


class TransitiveUpstream(unittest.TestCase):
    def setUp(self):
        self.profile=dict(name="app",version="1.0.0",features=[],default_features=True)
        before=["app@1.0.0","serde@1.0.0","syn@1.0.7"]
        after=["app@1.0.0","serde@1.0.1","syn@1.0.8"]
        def output(ids):
            return dict(status="ok",profiles=[self.profile],version_union=ids,
                        roots=[dict(profile=self.profile,resolved_versions=ids)])
        self.before,self.after=output(before),output(after)
        serde=encoded(name="serde",deps=[dict(name="syn",req="=1.0.7")])
        syn=encoded(name="syn",vers="1.0.7")+encoded(name="syn",vers="1.0.8")
        self.old={"3/a/app":encoded(name="app",deps=[dict(name="serde",req="^1.0")]),"se/rd/serde":serde,"3/s/syn":syn}
        self.new=dict(self.old,**{"se/rd/serde":serde+encoded(name="serde",vers="1.0.1",deps=[dict(name="syn",req="=1.0.8")])})
        self.baseline=[dict(profile=self.profile,cargo_versions=before,filtered_versions=before,native_only=[])]
        self.cargo=[dict(profile=self.profile,cargo_versions=after,filtered_versions=after,native_only=[],ok=True)]

    def classify(self,cargo=True):
        return corpus.classify(self.before,self.after,self.before,self.old,self.new,True,
                               self.baseline,self.cargo if cargo else None)

    def test_new_serde_changes_syn_without_syn_index_change_with_exact_cargo(self):
        self.assertEqual(self.old["3/s/syn"],self.new["3/s/syn"])
        self.assertEqual(self.classify()[0],"upstream")

    def test_without_cargo_requires_review_and_full_run(self):
        label,reasons=self.classify(False)
        self.assertEqual(label,"upstream_needs_review")
        self.assertIn("likely upstream; confirm with a full run",str(reasons))

    def test_root_outside_fast_subset_requires_review(self):
        self.cargo=[]
        self.assertEqual(self.classify()[0],"upstream_needs_review")

    def test_cargo_disagreement_is_regression(self):
        self.cargo[0]["cargo_versions"]=["app@1.0.0","serde@1.0.1","syn@1.0.7"]
        self.assertEqual(self.classify()[0],"regression")

    def test_before_cargo_disagreement_is_regression(self):
        self.baseline[0]["cargo_versions"]=["app@1.0.0"]
        self.assertEqual(self.classify()[0],"regression")

    def test_filtered_cargo_disagreement_is_regression(self):
        self.cargo[0]["filtered_versions"]=["app@1.0.0"]
        self.assertEqual(self.classify()[0],"regression")

    def test_missing_expected_cargo_selection_requires_review(self):
        del self.baseline[0]["cargo_versions"]
        self.assertEqual(self.classify()[0],"upstream_needs_review")

    def test_identity_mutation_stays_regression_even_with_exact_cargo(self):
        self.new["3/s/syn"]=encoded(name="syn",vers="1.0.7",cksum="1"*64)+encoded(name="syn",vers="1.0.8")
        self.assertEqual(self.classify()[0],"regression")

    def shared_unchanged_root(self):
        profile = dict(name="syn", version="3.0.10", features=[], default_features=True)
        shared = ["proc-macro2@1.0.0", "quote@1.0.0", "syn@3.0.10"]
        for value in (self.before, self.after):
            value["roots"][0]["resolved_versions"] = sorted(set(value["version_union"] + shared))
            value["profiles"].append(profile)
            value["roots"].append(dict(profile=profile, resolved_versions=shared))
            value["version_union"] = sorted(set(value["version_union"] + shared))
        for value, parent in ((self.before, "serde@1.0.0"), (self.after, "serde@1.0.1")):
            value["version_details"] = {v:dict(reasons=[dict(root=0,parent=parent),dict(root=1,parent="syn@3.0.10")]) for v in shared[:2]}
        for path, data in {"pr/oc/proc-macro2":encoded(name="proc-macro2"), "qu/ot/quote":encoded(name="quote")}.items():
            self.old[path] = self.new[path] = data
        self.old["3/s/syn"] += encoded(name="syn",vers="3.0.10")
        self.new["3/s/syn"] = self.old["3/s/syn"]
        for rows, value in ((self.baseline,self.before),(self.cargo,self.after)):
            rows[0]["cargo_versions"] = value["roots"][0]["resolved_versions"]
            rows[0]["filtered_versions"] = value["roots"][0]["resolved_versions"]
            rows.append(dict(profile=profile,cargo_versions=shared,filtered_versions=shared,native_only=[],ok=True))

    def test_shared_reasons_do_not_mark_unchanged_syn_root_as_changed(self):
        self.shared_unchanged_root()
        self.assertEqual(self.before["roots"][1], self.after["roots"][1])
        self.assertEqual(self.classify()[0], "upstream")
        self.assertTrue(any(isinstance(d,dict) and d.get("kind")=="informational" for d in self.classify()[1]))

    def test_membership_changes_are_checked_only_for_the_changed_root(self):
        self.shared_unchanged_root()
        self.after["roots"][0]["resolved_versions"].remove("proc-macro2@1.0.0")
        # Still selected by the unchanged syn root; the whole-corpus union keeps it.
        self.assertIn("proc-macro2@1.0.0",self.after["version_union"])
        self.assertEqual(self.classify()[0], "upstream")
        unrelated = dict(self.old, **{"1/y": encoded(name="y")})
        self.assertEqual(corpus.classify(self.before,self.after,self.before,self.old,unrelated,True,
                                       self.baseline,self.cargo)[0], "regression")

    def test_shared_reason_drift_still_needs_cargo_for_real_selection_change(self):
        self.shared_unchanged_root()
        self.assertEqual(self.classify(False)[0], "upstream_needs_review")

    def test_proposal_updates_cargo_proof_and_preserves_allowance(self):
        doc={"oracle_native_only":self.baseline}
        proposed=corpus.proposal_oracle(doc,self.cargo)
        self.assertEqual(proposed[0]["cargo_versions"],self.after["version_union"])
        self.assertEqual(proposed[0]["native_only"],self.baseline[0]["native_only"])
        self.assertEqual(doc["oracle_native_only"][0]["cargo_versions"],self.before["version_union"])

    def test_unrelated_or_format_only_change_cannot_excuse_transitive_change(self):
        for new in [dict(self.old,**{"1/y":encoded(name="y")}),
                    dict(self.old,**{"se/rd/serde":self.old["se/rd/serde"].replace(b": ",b":")})]:
            self.assertEqual(corpus.classify(self.before,self.after,self.before,self.old,new,True,
                                           self.baseline,self.cargo)[0],"regression")


class FastOracleEvidence(unittest.TestCase):
    def setUp(self):
        self.before = dict(status="ok", version_union=["a@1.0.0", "b@1.0.0", "c@1.0.0"],
            roots=[dict(profile=dict(name=name,version="1.0.0"),resolved_versions=[name+"@1.0.0"]) for name in "abc"])
        self.after = copy.deepcopy(self.before)

    def test_changed_root_outside_sample_is_qualified_with_its_original_index(self):
        self.after["version_union"].append("c@1.1.0")
        self.after["roots"][2]["resolved_versions"] = ["c@1.1.0"]
        self.assertEqual(corpus.fast_oracle_roots(self.before,self.after,limit=1),{0,2})

    def test_membership_change_with_unchanged_union_gets_cargo_evidence(self):
        self.after["roots"][2]["resolved_versions"].append("a@1.0.0")
        self.assertEqual(corpus.fast_oracle_roots(self.before,self.after,limit=1),{0,2})

    def test_reason_only_drift_does_not_expand_sample(self):
        self.after["version_details"] = {"c@1.0.0":dict(reasons=["new explanation"])}
        self.assertEqual(corpus.fast_oracle_roots(self.before,self.after,limit=1),{0})

    def test_oracle_preserves_noncontiguous_original_root_indices(self):
        profiles = [r["profile"] for r in self.before["roots"]]
        plan = dict(roots=[dict(profile=p) for p in profiles],
                    versions=[dict(name=name,version="1.0.0",index_line=encoded(name=name).decode(),
                                   reasons=[dict(root=n)],expected_bytes=None) for n,name in enumerate("abc")],
                    witnesses=[dict(versions=[name+"@1.0.0"]) for name in "abc"])
        with tempfile.TemporaryDirectory() as directory:
            work=Path(directory)
            with patch.object(corpus,"native_case",return_value=(dict(status="ok",version_union=["c@1.0.0"]),None,{},0,None)) as native, \
                 patch.object(corpus,"project"), patch.object(corpus,"cargo_environment",return_value={}), \
                 patch.object(corpus.subprocess,"run",return_value=subprocess.CompletedProcess([],0,"","")), \
                 patch.object(corpus,"lock_ids",return_value={"c@1.0.0"}):
                ok,rows=corpus.oracle(dict(roots=profiles),plan,work,Mock(base="https://fixture.invalid"),Path("binary"),{2})
            self.assertTrue(ok)
            self.assertEqual(native.call_args.args[2],work/"native-root-2")
            self.assertEqual([r["profile"]["name"] for r in rows],["c"])
            self.assertEqual(rows[0]["cargo_versions"],["c@1.0.0"])
            self.assertEqual(rows[0]["native_only"],[])
            self.assertEqual(rows[0]["witness_versions"],["c@1.0.0"])

    def test_missing_baseline_or_changed_profiles_qualify_every_root(self):
        self.assertIsNone(corpus.fast_oracle_roots(None,self.after))
        self.after["roots"][2]["profile"]["version"]="2.0.0"
        self.assertIsNone(corpus.fast_oracle_roots(self.before,self.after))



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


def owned(number=3, **changes):
    return dict(number=number, title=corpus.ISSUE_TITLE, state="open",
        user={"login":corpus.ISSUE_CREATOR}, labels=[{"name":corpus.ISSUE_LABEL}],
        body="Human context\n"+corpus.ISSUE_MARKER+"old") | changes


class TrackingIssue(unittest.TestCase):
    def test_clean_does_not_create_an_issue(self):
        api=Mock(side_effect=[{},[]])
        self.assertEqual(corpus.track_report("owner/repo",dict(failed=False,changed=False),"clean",api),"clean")
        self.assertEqual(api.call_count,2)
        query=api.call_args.args[2]
        self.assertIn("labels="+corpus.ISSUE_LABEL,query)
        self.assertIn("creator=github-actions%5Bbot%5D",query)

    def test_missing_label_is_created_before_filtered_query(self):
        missing=urllib.error.HTTPError("url",404,"missing",{},None)
        api=Mock(side_effect=[missing,{},[],{}])
        self.assertEqual(corpus.track_report("owner/repo",dict(changed=True),"changed",api),"opened")
        self.assertEqual(api.call_args_list[1].args[2],"labels")
        self.assertEqual(api.call_args.args[3]["labels"],[corpus.ISSUE_LABEL])
        self.assertIn(corpus.ISSUE_MARKER,api.call_args.args[3]["body"])

    def test_failure_reopens_owned_issue_and_preserves_human_context(self):
        api=Mock(side_effect=[{},[owned(state="closed")],{}])
        self.assertEqual(corpus.track_report("owner/repo",dict(failed=True),"failure",api),"updated")
        self.assertEqual(api.call_args.args[2],"issues/3")
        self.assertEqual(api.call_args.args[3]["state"],"open")
        self.assertTrue(api.call_args.args[3]["body"].startswith("Human context"))

    def test_clean_closes_only_owned_issue(self):
        api=Mock(side_effect=[{},[owned(7)],{}])
        self.assertEqual(corpus.track_report("owner/repo",dict(failed=False,changed=False),"clean",api),"closed")
        self.assertEqual(api.call_args.args[2],"issues/7")

    def test_human_issue_missing_marker_wrong_label_and_pr_are_never_changed(self):
        invalid=[owned(user={"login":"owner"}),owned(body="same title"),
                 owned(labels=[]),owned(pull_request={"url":"pr"})]
        for failed in [False,True]:
            for item in invalid:
                with self.subTest(failed=failed,item=item):
                    api=Mock(side_effect=[{},[item],{}])
                    corpus.track_report("owner/repo",dict(failed=failed),"report",api)
                    self.assertFalse(any(c.args[0]=="PATCH" for c in api.call_args_list))
                    self.assertEqual(api.call_count,3 if failed else 2)

    def test_duplicate_owned_issues_refuse(self):
        api=Mock(side_effect=[{},[owned(1),owned(2)]])
        with self.assertRaisesRegex(ValueError,"multiple"):
            corpus.track_report("owner/repo",dict(failed=True),"bad",api)
        self.assertEqual(api.call_count,2)

    def test_pagination_continues_past_1000_filtered_results(self):
        ignored=[owned(body="no managed marker") for _ in range(100)]
        api=Mock(side_effect=[{},*([ignored]*11),[owned(99)],{}])
        self.assertEqual(corpus.track_report("owner/repo",{},"clean",api),"closed")
        self.assertIn("page=12",api.call_args_list[-2].args[2])
        self.assertEqual(api.call_args.args[2],"issues/99")

    def test_needs_review_issue_wording_remains_failing_and_distinct(self):
        api=Mock(side_effect=[{},[],{}])
        corpus.track_report("owner/repo",dict(failed=True,upstream_needs_review=True),"review",api)
        self.assertIn("likely upstream; confirm with a full run",api.call_args.args[3]["body"])
        self.assertIn("fail qualification",api.call_args.args[3]["body"])

    def test_label_creation_race_rechecks_exact_label(self):
        api=Mock(side_effect=[urllib.error.HTTPError("url",404,"missing",{},None),
            urllib.error.HTTPError("url",422,"exists",{},None),{},[]])
        self.assertEqual(corpus.track_report("owner/repo",{},"clean",api),"clean")
        self.assertEqual(api.call_args_list[2].args[2],"labels/"+corpus.ISSUE_LABEL)


class ReleaseGate(unittest.TestCase):
    sha="a"*40
    previous="b"*40

    def weekly(self, **changes):
        return dict(id=1,status="completed",conclusion="success",head_sha=self.sha) | changes

    def test_weekly_on_exact_commit_is_accepted(self):
        api=Mock(return_value={"workflow_runs":[self.weekly()]})
        self.assertEqual(corpus.release_check("owner/repo",self.sha,api),1)
        self.assertEqual(api.call_count,1)

    def test_weekly_with_unchanged_watched_paths_is_accepted(self):
        api=Mock(return_value={"workflow_runs":[self.weekly(head_sha=self.previous)]})
        unchanged=Mock(return_value=True)
        self.assertEqual(corpus.release_check("owner/repo",self.sha,api,unchanged),1)
        unchanged.assert_called_once_with(self.previous,self.sha)

    def test_weekly_with_changed_watched_paths_requires_exact_dispatch(self):
        for dispatch in [[],[dict(id=3,head_sha=self.sha,status="completed",conclusion="success")]]:
            api=Mock(side_effect=[{"workflow_runs":[self.weekly(head_sha=self.previous)]},{"workflow_runs":dispatch}])
            if dispatch:
                self.assertEqual(corpus.release_check("owner/repo",self.sha,api,lambda *_:False),3)
            else:
                with self.assertRaisesRegex(ValueError,"corpus gate refused"):
                    corpus.release_check("owner/repo",self.sha,api,lambda *_:False)

    def test_missing_failed_incomplete_or_wrong_head_runs_refuse(self):
        for weekly in [[],[self.weekly(conclusion="failure")],[self.weekly(status="in_progress")]]:
            for dispatch in [[],[dict(id=2,head_sha=self.previous,status="completed",conclusion="success")],
                              [dict(id=2,head_sha=self.sha,status="completed",conclusion="failure")]]:
                with self.subTest(weekly=weekly,dispatch=dispatch):
                    api=Mock(side_effect=[{"workflow_runs":weekly},{"workflow_runs":dispatch}])
                    with self.assertRaisesRegex(ValueError,"corpus gate refused"):
                        corpus.release_check("owner/repo",self.sha,api)

    def test_git_diff_checks_actual_workflow_paths_without_api_file_limit(self):
        original_paths=corpus.watched_paths()
        with tempfile.TemporaryDirectory() as directory:
            repo=Path(directory)
            def git(*args):
                return subprocess.check_output(["git","-C",str(repo),"-c","user.name=Fixture","-c","user.email=fixture@example.invalid",*args],stderr=subprocess.DEVNULL,text=True).strip()
            git("init")
            workflow=repo/".github/workflows/corpus.yml"
            workflow.parent.mkdir(parents=True)
            workflow.write_text((corpus.REPO/".github/workflows/corpus.yml").read_text())
            (repo/"README.md").write_text("base")
            git("add",".");git("commit","-m","base");base=git("rev-parse","HEAD")
            (repo/"README.md").write_text("docs")
            git("add",".");git("commit","-m","docs");docs=git("rev-parse","HEAD")
            with patch.object(corpus,"REPO",repo):
                self.assertTrue(corpus.weekly_unchanged(base,docs))
                self.assertEqual(corpus.watched_paths(),original_paths)
                for path in ["src/cli/crates.rs","src/crates/new.rs","tests/corpus/new.json","scripts/corpus.py","scripts/check_crates_phase0.py","scripts/crates_phase0_fixtures.py",
                             "scripts/prove_crates_https.py","tests/crates_mirror.rs","tests/test_corpus.py","Cargo.lock"]:
                    previous=git("rev-parse","HEAD")
                    f=repo/path;f.parent.mkdir(parents=True,exist_ok=True);f.write_text("changed")
                    git("add",".");git("commit","-m","relevant")
                    self.assertFalse(corpus.weekly_unchanged(previous,git("rev-parse","HEAD")))
                self.assertFalse(corpus.weekly_unchanged("c"*40,docs))
                with patch.object(corpus,"watched_paths",side_effect=ValueError("unsupported")):
                    self.assertFalse(corpus.weekly_unchanged(base,docs))



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
