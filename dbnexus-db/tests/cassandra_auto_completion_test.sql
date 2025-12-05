ALTER ROLE role_test WITH PASSWORD = 'new_password' AND LOGIN = true AND SUPERUSER = false;
20 role_test
ALTER TABLE employees DROP department;
21 employees ; 38 department
ALTER TABLE employees rename  department to department1 ;
21 employees ; 41 department 
ALTER TYPE address ALTER zip_code TYPE text;
19 address ; 29 zip_code
ALTER TYPE address rename zip_code to zip_code;
19 address ; 34 zip_code
CREATE OR REPLACE AGGREGATE no_op_aggregate(int) SFUNC no_op_state_function STYPE int FINALFUNC no_op_final_function INITCOND 0;
76 no_op_state_function ; 114 no_op_final_function
CREATE INDEX index_name on employees (department);
37 employees ; 45 department
CREATE MATERIALIZED VIEW test.employees_by_department AS SELECT employee_id FROM employees  WHERE employee_id IS NOT NULL PRIMARY KEY (employee_id);
30 test ; 76 employee_id ; 91 employees ; 110 employee_id ; 147 employee_id
CREATE TABLE test.employees (  employee_id UUID PRIMARY KEY );
18 test
DELETE FROM test.employees where employee_id = 1
17 test ; 27 employees ; 45 employee_id
DROP AGGREGATE   test.no_op_aggregate;
22 test ; 38 no_op_aggregate
DROP FUNCTION test.no_op_state_function;
19 test ; 20 no_op_state_function
DROP INDEX IF EXISTS email_index;
33 email_index
DROP KEYSPACE IF EXISTS my_keyspace;
36 test
DROP MATERIALIZED VIEW IF EXISTS test.view_name;
38 test ; 48 employees_by_department
DROP ROLE IF EXISTS role_name;
30 role_test
DROP TABLE IF EXISTS test.employees;
26 test ; 36 employees
DROP TYPE test.completion_type;
15 test ; 31 completion_type
DROP ROLE IF EXISTS role_name;
30 role_test
GRANT SELECT, MODIFY ON test.employees TO role_name;
29 test ; 39 employees 
INSERT INTO test.employees(employee_id) VALUES (1);
17 test ; 27 employees ; 39 employee_id
REVOKE SELECT ON test.employees FROM role_name;
22 test ; 32 employees
SELECT employee_id from test.employees order by employee_id
19  abs; 19 employee_id ; 29 test ; 39 employees ; 60 employee_id
TRUNCATE test.employees;
14 test 
UPDATE test.employees SET employee_id = 1 WHERE employee_id = 2;
12 test ; 22 employees ; 38 employee_id ; 60 employee_id
use test;
9 test