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
insert into employees (id) values (1) RETURNING id
26 name ; 51 id 
insert into employees(id) select id from employees;
25 id ; 51 employees
select count(*) from employees
11 count ; 11 abs ; 11 sin
update employees set name = 1 where id = 10000 RETURNING id 
25 name ; 25 id ; 39 name ; 60 id 
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
delete from employees where id = 1000 RETURNING id
31 id ; 31 name ; 51 id
delete from employees USING employees
38 employees
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
30 do_nothing_procedure
drop POLICY no_op_policy on employees
25 no_op_policy ; 38 employees
SET ROLE role_name;
19 postgres
SET search_path TO my_schema;
16 search_path
DROP TRIGGER do_nothing_trigger on employees;
32 do_nothing_trigger ; 45 employees
select * from xxx.employees;
18 xxx ; 28 employees
GRANT UPDATE (id) ON employees TO user_role WITH GRANT OPTION  GRANTED BY current_user;
17 id ; 31 employees ; 44 postgres ; 87 postgres
PREPARE select_user AS SELECT * FROM employees WHERE id = $1;
47 employees
COPY employees(id) TO 'file.csv' WITH csv Header
15 employees ; 18 id 
COPY (select * from employees) to 'file.csv' WITH csv Header
30 employees
CREATE POLICY departments_policy ON departments FOR ALL USING (dept = current_setting('my.dept', true))  WITH CHECK (dept = current_setting('my.dept', true));
45 departments
CREATE EXTENSION hstore;
24 hstore
DROP EXTENSION plpgsql;
23 plpgsql
SELECT * FROM unnest(ARRAY[10,20,30]) AS numbers WHERE numbers > 0 ;
63 numbers
SELECT * from abs(-1) as abs WHERE abs > 0 
39 abs
COMMENT ON SCHEMA xxx IS '任意文字';
22 xxx
COMMENT ON DATABASE test IS NULL;
25 test
COMMENT ON TABLE employees IS '员工基本信息表';
27 employees
COMMENT ON COLUMN employees.id IS '税前月薪，单位：元';
28 employees ;31 id
COMMENT ON ROLE app_read IS NULL;
25 postgres
GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA xxx TO postgres;
49 xxx ; 49 test ;61 postgres 
GRANT ALL PRIVILEGES ON SEQUENCE xxx.do_nothing_seq TO postgres;
37 xxx ; 52 do_nothing_seq ; 64 postgres
GRANT ALL PRIVILEGES ON TABLE xxx.employees TO postgres;
34 xxx ; 44 employees ; 56 postgres
GRANT ALL PRIVILEGES ON DATABASE test TO postgres;
38 test ; 50 postgres
GRANT SELECT ON VIEW xxx.employees_test TO postgres;
25 xxx ; 40 employees_test ; 52 postgres
GRANT EXECUTE ON PROCEDURE xxx.do_nothing_procedure() TO postgres;
31 xxx ; 52 do_nothing_procedure ; 66 postgres
GRANT EXECUTE ON FUNCTION xxx.do_nothing() TO role_name;
30 xxx ; 41 do_nothing ; 56 postgres
create SEQUENCE xxx.abc
20 xxx
select * from (insert into employees(id) values (2) RETURNING id, id as xxx) WHERE id and xxx
37 employees ; 40 id ; 65 id ; 86 id ; 94 id 
select * from (insert into employees(id) values (2) RETURNING *) WHERE id
37 employees ; 40 id ; 74 id 
select * from (delete from employees where id = 1  RETURNING *) WHERE id
37 employees ; 46 id ; 73 id 
select * from (delete from employees as t where id = 1  RETURNING t.*) WHERE id
37 employees ; 51 id ; 80 id 
select * from (delete from employees where id = 1  RETURNING departments.*) WHERE id
37 employees ; 46 id ; 85 id 
SELECT * FROM (VALUES (1, 2), (3, 4)) AS t(c1, c2) WHERE c1 > 0 
60 c1
WITH aaa as (select * from employees) select * from employees where aaa.id > 0 
72 aaa ; 75 id 
WITH aaa(emp_id) as (select * from employees) select * from employees where aaa.emp_id > 0 
80 aaa ; 87 emp_id 
show allow_alter_system
24 allow_alter_system