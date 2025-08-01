TRUNCATE TABLE employees ON CLUSTER my_cluster;
24 employees ; 46 default
SELECT employee_id from employees
19 employee_id ; 34 employees
INSERT into employees(employee_id) VALUES (1)
34 employee_id
UPDATE employees set employee_id = 1 WHERE employee_id = 2
17 employees ; 33 employee_id ; 54 employee_id
DELETE FROM  employees WHERE employee_id = 1
23 employees ; 41 employee_id
CREATE VIEW employee_salaries AS SELECT employee_id ADD FROM employees;
51 employee_id
ALTER TABLE employees DROP PROJECTION employee_salary_projection;
22 employees