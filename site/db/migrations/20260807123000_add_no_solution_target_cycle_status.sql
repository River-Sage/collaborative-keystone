ALTER TABLE cycle_results
DROP CONSTRAINT cycle_results_status_check;

ALTER TABLE cycle_results
ADD CONSTRAINT cycle_results_status_check
    CHECK (result_status IN ('resolved', 'no_ranked_winner', 'no_solution_target'));
