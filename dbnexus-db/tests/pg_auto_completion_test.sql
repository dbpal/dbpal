truncate employees 
19 employees
select employees.id  from employees
13 employees ; 19 name ; 32 departments
SELECT e.id AS EmployeeID,  e.name AS EmployeeName,  d.name AS DepartmentName FROM employees AS e JOIN departments AS d ON e.department_id = d.id;
9 e ; 9 employees ; 12 name ; 54 d ; 54 departments ; 60 name ; 93 departments ; 125 e ; 125 departments ; 139 department_id ; 143 d ; 146 id
SELECT e.id AS EmployeeID,e.name AS EmployeeName,d.name AS DepartmentName FROM employees AS e JOIN departments AS d USING (department_id);
135 department_id
select id from (select id from employees)
10 id ; 25 name
select id from employees e 
10 name
insert into employees (id) select id from employees
18 employees ; 26 name ; 37 name
insert into employees (id) values (1)
26 name
select count(*) from employees
11 count ; 11 abs ; 11 sin
update employees set name = 1 where id = 10000
25 name ; 25 id ; 39 name
create view view_name as select * from employees
49 employees
COPY employees(id) FROM '/path/to/myfile.csv' WITH (FORMAT csv);
15 employees ; 18 id
update employees set name = 1 where id = 10000
25 name ; 25 id ; 39 name
update employees set name = 1 where id = 10000 RETURNING id 
25 name ; 25 id ; 39 name ; 60 id
update employees set name = 1 from employees where id = 10000
45 employees
delete from employees where id = 1000
31 id ; 31 name
delete from employees USING employees
38 employees
create view view_name as select * from employees
49 employees
CREATE TABLE order_items ( order_id INT, product_id INT, quantity INT, PRIMARY KEY (order_id, product_id));
93 order_id
CREATE TABLE child ( id SERIAL PRIMARY KEY, parent_id INT, FOREIGN KEY (parent_id) REFERENCES employees(id));
104 employees ; 107 id
CREATE TABLE products ( price NUMERIC CHECK (price >= 0));
51 price
CREATE INDEX idx_orders_status ON employees (id DESC) INCLUDE (id) WHERE id = 'shipped';
44 employees ; 48 id ; 66 id ; 76 id
CREATE ROLE my_role LOGIN PASSWORD 'secure_password' IN GROUP pg_database_owner;
80 pg_database_owner
CREATE POLICY order_access_policy  ON employees  FOR ALL  USING (id = current_user_id())  WITH CHECK (id = current_user_id());
48 employees ; 68 id ; 105 id
ALTER TABLE employees ADD CONSTRAINT unique_id UNIQUE (id);
22 employees ; 58 id
ALTER TABLE employees ADD CONSTRAINT fk_customer FOREIGN KEY (id) REFERENCES departments (id);
22 employees ; 65 id ; 89 departments ; 93 id
ALTER TABLE employees ADD CONSTRAINT check_id_positive CHECK (id > 0);
22 employees ; 65 id
ALTER TABLE employees DISABLE TRIGGER do_nothing_trigger;
57 do_nothing_trigger
ALTER TABLE employees RENAME COLUMN id TO id;
39 id
ALTER TABLE employees RENAME TO employees;
22 employees
ALTER TABLE employees ALTER COLUMN id SET DEFAULT 10000;;
38 id
ALTER TABLE employees RENAME CONSTRAINT employees_pkey TO employees_pkey;
55 employees_pkey
ALTER INDEX employees_pkey RENAME TO employees_pkey;
27 employees_pkey
ALTER ROLE role_name WITH PASSWORD 'new_password';
21 postgres
ALTER POLICY no_op_policy ON employees USING (role = 'manager');
26 no_op_policy ; 39 employees
drop table employees;
21 employees
DROP FUNCTION IF EXISTS do_nothing(INTEGER);
35 do_nothing
DROP PROCEDURE procedure_name() 
30 test_procedure
drop POLICY no_op_policy on employees
25 no_op_policy ; 38 employees
SET ROLE role_name;
19 postgres
SET search_path TO my_schema;
16 search_path
DROP TRIGGER trigger_insert_test_create_trigger on test_create_trigger;
48 trigger_insert_test_create_trigger ; 71 test_create_trigger
select * from xxx.employees;
18 xxx ; 28 employees
GRANT UPDATE (id) ON employees TO user_role WITH GRANT OPTION  GRANTED BY current_user;
17 id ; 31 employees ; 44 postgres ; 87 postgres
PREPARE select_user AS SELECT * FROM employees WHERE id = $1;
47 employees
COPY employees(id) TO 'file.csv' WITH csv Header
15 employees ; 18 id 
copy (select * from employees) to 'file.csv' WITH csv Header
30 employees
CREATE POLICY departments_policy ON departments FOR ALL USING (dept = current_setting('my.dept', true))  WITH CHECK (dept = current_setting('my.dept', true));
45 departments